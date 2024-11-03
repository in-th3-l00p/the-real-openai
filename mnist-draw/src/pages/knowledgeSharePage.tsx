// src/components/KnowledgeSharePage.tsx
import React, { useState, useEffect, useContext } from 'react';
import { BrowserProvider, Contract, BigNumber } from 'ethers';
import EthContext from '../context/EthContext';
import { KNOWLEDGE_ABI, KNOWLEDGE_PUBLISH } from '../utils/contracts';

type KnowledgeItem = {
  address: string;
  knowledge: string;
};

const KnowledgeSharePage: React.FC = () => {
  const { isAuthenticated, address, balance, setBalance } = useContext(EthContext);
  const [contract, setContract] = useState<Contract | null>(null);
  const [isRewardInProgress, setIsRewardInProgress] = useState<boolean>(false);
  const [knowledge, setKnowledge] = useState<string>('');
  const [submittedKnowledge, setSubmittedKnowledge] = useState<KnowledgeItem[]>([]);
  const [statusMessage, setStatusMessage] = useState<string>('');
  const [loading, setLoading] = useState<boolean>(true);
  const [voteCount, setVoteCount] = useState<number>(0);

  // Initialize contract and check reward status
  useEffect(() => {
    if (!isAuthenticated) window.location.href = '/';
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
          const result: any = await contractInstance.getSubmittedKnowledge();
          const addresses: string[] = result[0];
          const knowledgeList: string[] = result[1];
          const combinedList = addresses.map((addr, idx) => ({
            address: addr,
            knowledge: knowledgeList[idx],
          }));
          setSubmittedKnowledge(combinedList);

          // Fetch user's vote count
          const userVoteCount: BigNumber = await contractInstance.getVote();
          setVoteCount(userVoteCount.toNumber());
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
      const result: any = await contract.getSubmittedKnowledge();
      const addresses: string[] = result[0];
      const knowledgeList: string[] = result[1];
      const combinedList = addresses.map((addr, idx) => ({
        address: addr,
        knowledge: knowledgeList[idx],
      }));
      setSubmittedKnowledge(combinedList);
    } catch (error) {
      console.error('Error sharing knowledge:', error);
      setStatusMessage('Failed to share knowledge. Please try again.');
    }
  };

  // Handle voting
  const handleVote = async (index: number) => {
    if (!contract) {
      setStatusMessage('Smart contract is not initialized.');
      return;
    }

    try {
      setStatusMessage('Submitting your vote...');
      const tx = await contract.vote(index);
      await tx.wait();
      setStatusMessage('Vote submitted successfully!');

      // Update vote count
      const updatedVoteCount: BigNumber = await contract.getVote();
      setVoteCount(updatedVoteCount.toNumber());
    } catch (error) {
      console.error('Error submitting vote:', error);
      setStatusMessage('Failed to submit vote. Please try again.');
    }
  };

  // Handle wallet disconnection (UI only)
  const disconnectWallet = () => {
    // Note: MetaMask doesn't support programmatic disconnection
    // This will only reset the UI state
    setBalance(0n);
    window.location.reload(); // Reload to reset connection
  };

  if (loading) {
    return (
      <div className="w-screen min-h-screen flex items-center justify-center bg-gradient-to-r from-blue-500 to-purple-600">
        <div className="text-center">
          <svg
            className="animate-spin h-12 w-12 text-white mx-auto"
            xmlns="http://www.w3.org/2000/svg"
            fill="none"
            viewBox="0 0 24 24"
          >
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
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
  }

  return (
    <div className="w-screen min-h-screen flex items-center justify-center bg-gradient-to-r from-blue-500 to-purple-600 p-4">
      <div className="bg-white bg-opacity-90 shadow-lg rounded-lg p-8 w-full max-w-2xl">
        {isAuthenticated && address ? (
          <>
            <div className="mb-6 text-center">
              <h2 className="text-3xl font-bold text-gray-800">Share Your Knowledge</h2>
              <p className="text-gray-600 mt-2">Contribute to the community by sharing your insights.</p>
              <p className="text-gray-600 mt-2">You have cast {voteCount} votes.</p>
            </div>
            {isRewardInProgress ? (
              <div className="bg-yellow-100 text-yellow-800 p-4 rounded mb-6 text-center">
                <strong>Rewards are currently being processed.</strong> Please try sharing your knowledge later.
              </div>
            ) : (
              <form onSubmit={handleSubmit} className="space-y-6">
                <textarea
                  value={knowledge}
                  onChange={handleKnowledgeChange}
                  rows={6}
                  className="w-full px-4 py-3 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                  placeholder="Enter your knowledge here..."
                  required
                ></textarea>
                <button
                  type="submit"
                  className="w-full bg-blue-600 text-white py-3 rounded-md hover:bg-blue-700 transition-colors font-semibold text-lg"
                >
                  Submit Knowledge
                </button>
              </form>
            )}
            {statusMessage && (
              <div className="mt-6 p-4 bg-green-100 text-green-800 rounded-md text-center">
                {statusMessage}
              </div>
            )}
            {/* Display Submitted Knowledge */}
            <div className="mt-8">
              <h3 className="text-2xl font-semibold text-gray-800 mb-4">Submitted Knowledge</h3>
              {submittedKnowledge.length > 0 ? (
                <ul className="space-y-4 max-h-64 overflow-y-auto pr-2">
                  {submittedKnowledge.map((item, index) => (
                    <li
                      key={index}
                      className="p-4 bg-gray-100 rounded-md shadow-sm text-gray-800 border-l-4 border-blue-500"
                    >
                      <p>{item.knowledge}</p>
                      <p className="text-sm text-gray-600">Shared by: {item.address}</p>
                      <button
                        onClick={() => handleVote(index)}
                        className="mt-2 bg-green-500 text-white py-1 px-3 rounded-md hover:bg-green-600 transition-colors"
                      >
                        Vote
                      </button>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="text-gray-600">No knowledge has been shared yet.</p>
              )}
            </div>
            <div className="mt-8 flex justify-center">
              <button
                onClick={disconnectWallet}
                className="bg-red-500 text-white py-2 px-6 rounded-md hover:bg-red-600 transition-colors font-semibold"
              >
                Disconnect Wallet
              </button>
            </div>
          </>
        ) : (
          <div className="text-center">
            <p className="mb-6 text-xl text-gray-800">Not connected to an Ethereum wallet.</p>
            <button
              onClick={() => window.location.reload()} // Trigger wallet connection
              className="w-full bg-blue-600 text-white py-3 rounded-md hover:bg-blue-700 transition-colors font-semibold text-lg"
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
