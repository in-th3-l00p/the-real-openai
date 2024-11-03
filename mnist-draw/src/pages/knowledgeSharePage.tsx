// src/components/KnowledgeSharePage.tsx
import React, { useState, useEffect, useContext } from 'react';
import { BrowserProvider, Contract } from 'ethers';
import EthContext from '../context/EthContext';
import { KNOWLEDGE_ABI, KNOWLEDGE_PUBLISH } from '../utils/contracts';

const KnowledgeSharePage: React.FC = () => {
  const { isAuthenticated, address, balance, setBalance } = useContext(EthContext);
  const [contract, setContract] = useState<Contract | null>(null);
  const [isRewardInProgress, setIsRewardInProgress] = useState<boolean>(false);
  const [knowledge, setKnowledge] = useState<string>('');
  const [submittedKnowledge, setSubmittedKnowledge] = useState<string[]>([]);
  const [statusMessage, setStatusMessage] = useState<string>('');
  const [loading, setLoading] = useState<boolean>(true);

  // Initialize contract and check reward status
  useEffect(() => {
    if (!isAuthenticated)
        window.location.href = "/";
    const init = async () => {
      if (isAuthenticated && address) {
        try {
          const provider = new BrowserProvider(window.ethereum);
          const signer = await provider.getSigner();
          const contractInstance = new Contract(KNOWLEDGE_PUBLISH, KNOWLEDGE_ABI, signer);
          setContract(contractInstance);

          // Check if reward is in progress
          const rewardStatus: boolean = await contractInstance.isRewardInProgress();
          setIsRewardInProgress(rewardStatus);

          // Fetch submitted knowledge
          const knowledgeList: string[] = await contractInstance.getSubmittedKnowledge();
          setSubmittedKnowledge(knowledgeList);

          // Optionally, listen for new knowledge submissions
          // contractInstance.on('KnowledgeShared', handleNewKnowledge);
        } catch (error) {
          console.error('Error initializing contract:', error);
        }
      }
      setLoading(false);
    };
    init();
  }, [isAuthenticated, address]);

  // Handle knowledge input change
  const handleKnowledgeChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setKnowledge(e.target.value);
  };

  // Handle form submission
  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!knowledge.trim()) {
      setStatusMessage('Please enter some knowledge to share.');
      return;
    }

    if (!contract) {
      setStatusMessage('Smart contract is not initialized.');
      return;
    }

    try {
      setStatusMessage('Submitting knowledge to the blockchain...');
      const tx = await contract.share(knowledge);
      await tx.wait();
      setStatusMessage('Knowledge shared successfully!');
      setKnowledge('');

      // Update submitted knowledge
      const knowledgeList: string[] = await contract.getSubmittedKnowledge();
      setSubmittedKnowledge(knowledgeList);
    } catch (error) {
      console.error('Error sharing knowledge:', error);
      setStatusMessage('Failed to share knowledge. Please try again.');
    }
  };

  // Handle wallet disconnection (UI only)
  const disconnectWallet = () => {
    // Note: MetaMask doesn't support programmatic disconnection
    // This will only reset the UI state
    setBalance(0n);
    // Additional state resets can be done here if necessary
    window.location.reload(); // Reload to reset connection
  };

  if (loading) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <p className="text-xl">Loading...</p>
      </div>
    );
  }

  return (
    <div className="min-h-screen flex items-center justify-center p-4">
      <div className="bg-white shadow-md rounded-lg p-8 w-full max-w-md">
        {isAuthenticated && address ? (
          <>
            <div className="mb-4">
              <p className="text-green-600 font-semibold">Connected to wallet:</p>
              <p className="text-gray-700 break-all">{address}</p>
              <p className="text-gray-700">Balance: {Number(balance) / 1e18} ETH</p>
            </div>
            {isRewardInProgress ? (
              <div className="bg-yellow-100 text-yellow-800 p-4 rounded mb-4">
                <strong>Rewards are currently being processed.</strong> Please try sharing your knowledge later.
              </div>
            ) : (
              <form onSubmit={handleSubmit} className="space-y-4">
                <h2 className="text-2xl font-bold text-center">Share Your Knowledge</h2>
                <textarea
                  value={knowledge}
                  onChange={handleKnowledgeChange}
                  rows={4}
                  className="w-full px-3 py-2 border rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                  placeholder="Enter your knowledge here..."
                  required
                ></textarea>
                <button
                  type="submit"
                  className="w-full bg-blue-500 text-white py-2 px-4 rounded-md hover:bg-blue-600 transition-colors"
                >
                  Submit
                </button>
              </form>
            )}
            {statusMessage && (
              <div className="mt-4 p-2 bg-gray-200 text-gray-800 rounded">
                {statusMessage}
              </div>
            )}
            {/* Display Submitted Knowledge */}
            <div className="mt-6">
              <h3 className="text-xl font-semibold mb-2">Submitted Knowledge</h3>
              {submittedKnowledge.length > 0 ? (
                <ul className="list-disc list-inside space-y-2">
                  {submittedKnowledge.map((item, index) => (
                    <li key={index} className="text-gray-800">
                      {item}
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="text-gray-600">No knowledge has been shared yet.</p>
              )}
            </div>
            <button
              onClick={disconnectWallet}
              className="mt-6 w-full bg-red-500 text-white py-2 px-4 rounded-md hover:bg-red-600 transition-colors"
            >
              Disconnect Wallet
            </button>
          </>
        ) : (
          <div className="text-center">
            <p className="mb-4">Not connected to an Ethereum wallet.</p>
            <button
              onClick={() => window.location.reload()} // Trigger wallet connection
              className="w-full bg-blue-500 text-white py-2 px-4 rounded-md hover:bg-blue-600 transition-colors"
            >
              Connect Ethereum Wallet
            </button>
          </div>
        )}
      </div>
    </div>
  );
};

export default KnowledgeSharePage;
