import { useState, useEffect } from "react";
import { ethers, parseUnits, formatEther } from "ethers";
import { ABI_TALENT, AUTHORIZATION } from "../utils/contracts";

export default function PurchasePage() {
  const [isAuthenticated, setIsAuthenticated] = useState<boolean>(false);
  const [address, setAddress] = useState<string | null>(null);
  const [balance, setBalance] = useState<bigint | null>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [contract, setContract] = useState<ethers.Contract | null>(null);
  const [purchasing, setPurchasing] = useState<boolean>(false);

  const CONTRACT_ADDRESS = AUTHORIZATION;
  const ABI = ABI_TALENT;
  const ACCESS_COST = 2180330000000000n; // Cost per access in wei

  const etherCost = ethers.formatEther(ACCESS_COST);

  useEffect(() => {
    const initialize = async () => {
      if (window.ethereum) {
        const provider = new ethers.BrowserProvider(window.ethereum);
        const accounts = await provider.listAccounts();
        if (accounts.length > 0) {
          setIsAuthenticated(true);
          const signer = await provider.getSigner();
          const userAddress = await signer.getAddress();
          setAddress(userAddress);

          // Create contract instance with signer for write operations
          const contractInstance = new ethers.Contract(CONTRACT_ADDRESS, ABI, signer);
          setContract(contractInstance);

          // Fetch user's balance
          const userBalance = await contractInstance.balanceOf(userAddress);
          setBalance(userBalance);
        } else {
          setIsAuthenticated(false);
          setAddress(null);
          setContract(null);
          setBalance(null);
        }
      } else {
        alert("Please install MetaMask to use this feature.");
      }
      setLoading(false);
    };
    initialize();
  }, []);

  const handlePurchase = async () => {
    if (!contract) return;
    setPurchasing(true);
    try {
      const tx = await contract.purchase({ value: ACCESS_COST });
      await tx.wait();

      // Update balance after purchase
      const userBalance = await contract.balanceOf(address);
      setBalance(userBalance);
      alert("Purchase successful!");
      window.location.reload();
    } catch (error) {
      console.error("Purchase failed:", error);
      alert("Purchase failed. Please try again.");
    }
    setPurchasing(false);
  };

  const handleConnectWallet = async () => {
    try {
      if (!window.ethereum) {
        alert("Please install MetaMask to use this feature.");
        return;
      }

      const provider = new ethers.BrowserProvider(window.ethereum);
      await provider.send("eth_requestAccounts", []);
      const signer = await provider.getSigner();
      const userAddress = await signer.getAddress();
      setIsAuthenticated(true);
      setAddress(userAddress);

      const contractInstance = new ethers.Contract(CONTRACT_ADDRESS, ABI, signer);
      setContract(contractInstance);

      const userBalance = await contractInstance.balanceOf(userAddress);
      setBalance(userBalance);
    } catch (error) {
      console.error("Wallet connection failed:", error);
      setIsAuthenticated(false);
    }
  };

  if (loading)
    return (
      <div className="w-screen min-h-screen flex items-center justify-center bg-gradient-to-r from-purple-500 to-indigo-500">
        <div className="flex flex-col items-center">
          <svg
            className="animate-spin h-12 w-12 text-white"
            xmlns="http://www.w3.org/2000/svg"
            fill="none"
            viewBox="0 0 24 24"
          >
            <circle
              className="opacity-25"
              cx="12"
              cy="12"
              r="10"
              stroke="currentColor"
              strokeWidth="4"
            ></circle>
            <path
              className="opacity-75"
              fill="currentColor"
              d="M4 12a8 8 0 018-8v4l5-5-5-5v4a10 10 0 100 20v-4l-5 5 5 5v-4a8 8 0 01-8-8z"
            ></path>
          </svg>
          <p className="text-white text-2xl mt-4">Loading...</p>
        </div>
      </div>
    );

  return (
    <div className="w-screen min-h-screen flex flex-col justify-center items-center bg-gradient-to-r from-purple-500 to-indigo-500 font-sans">
      <div className="bg-white p-8 rounded-lg shadow-lg flex flex-col items-center gap-6 text-center">
        {isAuthenticated ? (
          <div className="flex flex-col items-center gap-4">
            <p className="text-2xl font-semibold text-gray-800">
              Current Balance: {balance ? balance.toString() : "0"} Usages
            </p>
            <p className="text-gray-600">Connected Wallet: {address}</p>
            <p className="text-gray-600">Cost per access: {etherCost} ETH</p>
            <button
              type="button"
              onClick={handlePurchase}
              disabled={purchasing}
              className="px-6 py-3 bg-blue-600 text-white rounded-lg shadow-md hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:opacity-50"
            >
              {purchasing ? "Purchasing..." : "Purchase Access"}
            </button>
          </div>
        ) : (
          <div className="flex flex-col items-center gap-6">
            <p className="text-xl text-gray-800">Not connected to an Ethereum wallet.</p>
            <p className="text-gray-600">
              Connect your Ethereum wallet to purchase access.
            </p>
            <button
              type="button"
              onClick={handleConnectWallet}
              className="px-6 py-3 bg-green-600 text-white rounded-lg shadow-md hover:bg-green-700 focus:outline-none focus:ring-2 focus:ring-green-500"
            >
              Connect Wallet
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
