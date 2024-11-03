// src/components/ChatPage.tsx
import React, { useState, useContext, useEffect, useRef } from 'react';
import EthContext from '../context/EthContext';

interface Message {
  sender: 'user' | 'ai';
  content: string;
}

const ChatPage: React.FC = () => {
  const { isAuthenticated, address, balance, setBalance } = useContext(EthContext);
  const [inputMessage, setInputMessage] = useState('');
  const [chatHistory, setChatHistory] = useState<Message[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Scroll to bottom when new messages are added
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [chatHistory]);

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setInputMessage(e.target.value);
  };

  const handleSendMessage = async () => {
    if (!inputMessage.trim()) return;

    if (!isAuthenticated || !address) {
      setError('Please connect your Ethereum wallet to continue.');
      return;
    }

    if (balance === 0n) {
      setError('Insufficient balance to send message.');
      return;
    }

    setError('');
    setLoading(true);

    // Add user's message to chat history
    setChatHistory((prev) => [...prev, { sender: 'user', content: inputMessage }]);

    try {
      const response = await fetch('http://localhost:8000/query-ai', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify({
          query: inputMessage,
          ethAddress: address,
        }),
      });

      setBalance((balance) => (balance ? balance - 1n : 0n));
      if (!response.ok) {
        const errorData = await response.json();
        const errorMessage = errorData.error || 'An error occurred. Please try again.';
        throw new Error(errorMessage);
      }

      const data = await response.json();
      const aiResponse = data.result;

      // Add AI's response to chat history
      setChatHistory((prev) => [...prev, { sender: 'ai', content: aiResponse }]);
    } catch (err: any) {
      console.error('Error:', err);

      const errorMessage = err.message || 'An error occurred. Please try again.';
      setError(errorMessage);

      // Optionally add error message to chat history
      setChatHistory((prev) => [
        ...prev,
        { sender: 'ai', content: 'Error: ' + errorMessage },
      ]);
    } finally {
      setLoading(false);
      setInputMessage('');
    }
  };

  const handleKeyPress = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSendMessage();
    }
  };

  return (
    <section className="bg-zinc-200 w-screen h-screen flex justify-center items-center">
      <div className="flex flex-col w-full max-w-md h-[700px] bg-gradient-to-r from-purple-500 to-indigo-600 rounded-md mt-8 shadow-md">
        {/* Header */}
        <div className="flex items-center justify-between p-4 bg-white shadow-md">
          <h1 className="text-2xl font-bold text-gray-800">AI Chat</h1>
          <div className="text-gray-600">
            Balance: {balance ? balance.toString() : '0'} Tokens
          </div>
        </div>

        {/* Chat History */}
        <div className="flex-1 overflow-y-auto p-4">
          {chatHistory.map((message, index) => (
            <div
              key={index}
              className={`flex mb-4 ${
                message.sender === 'user' ? 'justify-end' : 'justify-start'
              }`}
            >
              <div
                className={`max-w-xs md:max-w-md px-4 py-2 rounded-lg shadow ${
                  message.sender === 'user'
                    ? 'bg-blue-500 text-white'
                    : 'bg-white text-gray-800'
                }`}
              >
                {message.content}
              </div>
            </div>
          ))}
          <div ref={messagesEndRef} />
        </div>

        {/* Error Message */}
        {error && (
          <div className="px-4 py-2 bg-red-500 text-white text-center">
            {error}
          </div>
        )}

        {/* Input Field */}
        <div className="p-4 bg-white flex">
          <input
            type="text"
            value={inputMessage}
            onChange={handleInputChange}
            onKeyPress={handleKeyPress}
            className="flex-1 px-4 py-2 border border-gray-300 rounded-l-md focus:outline-none focus:ring-2 focus:ring-blue-500"
            placeholder="Type your message..."
            disabled={!isAuthenticated || loading}
          />
          <button
            onClick={handleSendMessage}
            className="bg-blue-500 text-white px-6 py-2 rounded-r-md hover:bg-blue-600 disabled:opacity-50"
            disabled={!isAuthenticated || loading || balance === 0n}
          >
            {loading ? 'Sending...' : 'Send'}
          </button>
        </div>
      </div>
    </section>
    
  );
};

export default ChatPage;
