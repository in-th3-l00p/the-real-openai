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

      setBalance(balance => (balance ? balance - 1n : 0n))
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
    <div className="flex flex-col h-full p-4">
      {/* Chat History */}
      <div className="flex-1 overflow-y-auto mb-4">
        {chatHistory.map((message, index) => (
          <div
            key={index}
            className={`mb-2 ${message.sender === 'user' ? 'text-right' : 'text-left'}`}
          >
            <div
              className={`inline-block px-4 py-2 rounded-lg ${
                message.sender === 'user'
                  ? 'bg-blue-600 text-white'
                  : 'bg-gray-700 text-white'
              }`}
            >
              {message.content}
            </div>
          </div>
        ))}
        <div ref={messagesEndRef} />
      </div>

      {/* Error Message */}
      {error && <div className="text-red-500 mb-2">{error}</div>}

      {/* Input Field */}
      <div className="flex items-center">
        <input
          type="text"
          value={inputMessage}
          onChange={handleInputChange}
          onKeyPress={handleKeyPress}
          className="flex-1 px-4 py-2 rounded-l-lg bg-gray-800 text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
          placeholder="Type your message..."
          disabled={!isAuthenticated || loading}
        />
        <button
          onClick={handleSendMessage}
          className="bg-blue-600 text-white px-4 py-2 rounded-r-lg hover:bg-blue-700 disabled:opacity-50"
          disabled={!isAuthenticated || loading || balance === 0n}
        >
          {loading ? 'Sending...' : 'Send'}
        </button>
      </div>
    </div>
  );
};

export default ChatPage;
