import React from 'react';
import { useAccount, useConnect, useDisconnect } from 'wagmi';
import { injected } from 'wagmi/connectors';

export function Web3Button() {
  const { address, isConnected } = useAccount();
  const { connect } = useConnect();
  const { disconnect } = useDisconnect();

  if (isConnected) {
    return (
      <button
        onClick={() => disconnect()}
        className="px-4 py-2 font-semibold text-sm bg-blue-600 text-white rounded-lg shadow-sm hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-opacity-50"
      >
        Disconnect {address?.slice(0, 6)}...{address?.slice(-4)}
      </button>
    );
  }

  return (
    <button
      onClick={() => connect({ connector: injected() })}
      className="px-4 py-2 font-semibold text-sm bg-blue-600 text-white rounded-lg shadow-sm hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-opacity-50"
    >
      Connect Wallet
    </button>
  );
}