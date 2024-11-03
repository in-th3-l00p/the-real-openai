import { useState, useEffect } from "react";
import { ethers, parseUnits } from "ethers";
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

  if (loading) return <p className="text-white text-xl">Loading...</p>;

  return (
    <div className="w-screen min-h-screen flex flex-col justify-center items-center bg-zinc-700 gap-8 text-white">
      {isAuthenticated ? (
        <div className="flex flex-col items-center gap-4">
          <p>
            Current Balance: {balance ? balance.toString() : "0"} Usages
          </p>
          <button
            type="button"
            onClick={handlePurchase}
            disabled={purchasing}
            className="px-4 py-2 bg-blue-500 rounded"
          >
            {purchasing ? "Purchasing..." : "Purchase Access"}
          </button>
        </div>
      ) : (
        <div className="flex flex-col items-center gap-4">
          <p>Not connected to an Ethereum wallet.</p>
          <button
            type="button"
            onClick={handleConnectWallet}
            className="px-4 py-2 bg-blue-500 rounded"
          >
            Connect Wallet
          </button>
        </div>
      )}
    </div>
  );
}
