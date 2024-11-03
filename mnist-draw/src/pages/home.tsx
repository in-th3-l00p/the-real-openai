import React, { useContext } from 'react';
import { Link } from 'react-router-dom';
import { Brain, Users, Code, Database } from 'lucide-react';
import { ethers } from 'ethers';
import EthContext from '../context/EthContext';

function Home() {
  const { isAuthenticated } = useContext(EthContext);

  return (
    <div className="relative h-[calc(100vh-4rem)]">
      {/* Hero Section */}
      <div className="relative h-3/5 flex items-center">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="text-center">
            <h1 className="text-3xl tracking-tight font-extrabold text-gray-900 sm:text-4xl md:text-5xl">
              <span className="block">Democratizing AI,</span>
              <span className="block text-primary-600">One Digit at a Time</span>
            </h1>
            <p className="mt-3 max-w-md mx-auto text-base text-gray-500 sm:text-lg md:mt-4 md:text-xl md:max-w-3xl">
              Join us in building the future of AI through collaborative on-chain contributions. 
              Train MNIST models together, earn rewards, and be part of the decentralized AI revolution.
            </p>
            {isAuthenticated ? (
              <div className="mt-4 max-w-md mx-auto sm:flex sm:justify-center md:mt-6">
                <div className="rounded-md shadow">
                  <Link
                    to="/knowledge"
                    className="w-full flex items-center justify-center px-8 py-2 border border-transparent text-base font-medium rounded-md bg-primary-600 hover:bg-primary-700 md:text-lg md:px-10"
                  >
                    Knowledge
                  </Link>
                </div>
                <div className="mt-3 rounded-md shadow sm:mt-0 sm:ml-3">
                  <Link
                    to="/knowledge/share"
                    className="w-full flex items-center justify-center px-8 py-2 border border-transparent text-base font-medium rounded-md bg-primary-600 hover:bg-primary-700 md:text-lg md:px-10"
                  >
                    Knowledge Share
                  </Link>
                </div>
                <div className="mt-3 rounded-md shadow sm:mt-0 sm:ml-3">
                  <Link
                    to="/mnist"
                    className="w-full flex items-center justify-center px-8 py-2 border border-transparent text-base font-medium rounded-md  bg-primary-600 hover:bg-primary-700 md:text-lg md:px-10"
                  >
                    MNIST
                  </Link>
                </div>
                <div className="mt-3 rounded-md shadow sm:mt-0 sm:ml-3">
                  <Link
                    to="/purchase"
                    className="w-full flex items-center justify-center px-8 py-2 border border-transparent text-base font-medium rounded-md  bg-primary-600 hover:bg-primary-700 md:text-lg md:px-10"
                  >
                    Purchase
                  </Link>
                </div>
              </div>
            ) : (
              <div className="mt-4 max-w-md mx-auto sm:flex sm:justify-center md:mt-6">
                <button
                  type="button"
                  className="w-full flex items-center justify-center px-8 py-2 text-base font-medium rounded-md  bg-primary-600 hover:bg-primary-800 md:text-lg md:px-10"
                  onClick={async () => {
                    try {
                      if (!window.ethereum) {
                        alert('Please install MetaMask to use this feature.');
                        return;
                      }

                      await new ethers.BrowserProvider(window.ethereum);
                      window.location.reload();
                    } catch (error) {
                      console.error('Wallet connection failed:', error);
                    }
                  }}
                >
                  Authenticate
                </button>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Feature Section */}
      <div className="relative bg-gray-50 h-2/5">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 h-full flex items-center">
          <div className="grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-4 w-full">
            <div className="text-center">
              <div className="flex justify-center">
                <Brain className="h-10 w-10 text-zinc-600" />
              </div>
              <h3 className="mt-2 text-lg font-medium text-gray-900">Decentralized Learning</h3>
              <p className="mt-1 text-sm text-gray-500">
                Train AI models collaboratively using blockchain technology
              </p>
            </div>
            <div className="text-center">
              <div className="flex justify-center">
                <Users className="h-10 w-10 text-primary-600" />
              </div>
              <h3 className="mt-2 text-lg font-medium text-gray-900">Community Driven</h3>
              <p className="mt-1 text-sm text-gray-500">
                Contribute data and earn rewards for your participation
              </p>
            </div>
            <div className="text-center">
              <div className="flex justify-center">
                <Code className="h-10 w-10 text-primary-600" />
              </div>
              <h3 className="mt-2 text-lg font-medium text-gray-900">Transparent Code</h3>
              <p className="mt-1 text-sm text-gray-500">
                Open-source smart contracts built with Rust on Arbitrum
              </p>
            </div>
            <div className="text-center">
              <div className="flex justify-center">
                <Database className="h-10 w-10 text-primary-600" />
              </div>
              <h3 className="mt-2 text-lg font-medium text-gray-900">On-Chain Data</h3>
              <p className="mt-1 text-sm text-gray-500">
                Secure and verifiable training data stored on the blockchain
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default Home;
