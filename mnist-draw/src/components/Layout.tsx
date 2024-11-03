import { useState, useEffect } from "react";
import EthContext from "../context/EthContext";
import { ethers, formatUnits } from "ethers";
import { ABI_TALENT, AUTHORIZATION } from "../utils/contracts";

export default function Layout({ children }: { children: React.ReactNode }) {
  const [isAuthenticated, setIsAuthenticated] = useState<boolean>(false);
  const [address, setAddress] = useState<string | null>(null);
  const [balance, setBalance] = useState<bigint | null>(null);
  const [loading, setLoading] = useState<boolean>(true);

  useEffect(() => {
    const checkWalletConnection = async () => {
      if (window.ethereum) {
        const provider = new ethers.BrowserProvider(window.ethereum);
        const accounts = await provider.listAccounts();
        if (accounts.length > 0) {
          setIsAuthenticated(true);
          const signer = await provider.getSigner();
          const userAddress = await signer.getAddress();
          setAddress(userAddress);

          // Create contract instance and fetch balance
          const abi = ABI_TALENT;
          const contractAddress = AUTHORIZATION;
          const contract = new ethers.Contract(contractAddress, abi, provider);

          const userBalance = await contract.balanceOf(userAddress);
          setBalance(userBalance);
        }
      }
      setLoading(false);
    };
    checkWalletConnection();
  }, []);

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

      // Create contract instance and fetch balance
      const abi = [
        "function purchase() external payable returns (uint256)",
        "function balanceOf(address _address) external view returns (uint256)",
      ];
      const contractAddress = "0x104f5cc5d1593f1ba2a0eecf5882be85e231aca9";
      const contract = new ethers.Contract(contractAddress, abi, provider);

      const userBalance = await contract.balanceOf(userAddress);
      setBalance(userBalance);
    } catch (error) {
      console.error("Wallet connection failed:", error);
      setIsAuthenticated(false);
    }
  };

  const handleDisconnectWallet = async () => {
    try {
      if (!window.ethereum) {
        alert("Please install MetaMask to use this feature.");
        return;
      }

      new ethers.BrowserProvider(window.ethereum);
      window.location.reload();
    } catch (error) {
      console.error("Wallet connection failed:", error);
    }
  };

  if (loading) return <p className="text-white text-xl">Loading...</p>;

  return (
    <main className="w-screen min-h-screen flex justify-center items-center bg-zinc-700 gap-8">
      <div className="absolute top-0 right-0 text-white p-4">
        {isAuthenticated ? (
          <div className="flex flex-col items-end gap-2">
            <p>
              Connected to wallet: {address?.substring(0, 10)}
              {address && address.length > 10 ? "..." : ""}
            </p>
            <p>Balance: {balance ? balance.toString() : "0"} Tokens</p>
            <button type="button" onClick={handleDisconnectWallet}>
              Disconnect
            </button>
          </div>
        ) : (
          <div>
            <p>Not connected to an Ethereum wallet.</p>
            <button type="button" onClick={handleConnectWallet}>
              Connect Wallet
            </button>
          </div>
        )}
      </div>

      <EthContext.Provider value={{ 
        isAuthenticated, 
        balance: balance ? balance : 0n,
        setBalance: setBalance,
        address
      }}>
        {children}
      </EthContext.Provider>
    </main>
  );
}
