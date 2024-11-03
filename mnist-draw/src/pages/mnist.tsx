
// Mnist.jsx
import { useContext, useEffect, useState } from 'react';
import PixelGrid from '../components/PixelGrid';
import EthContext from '../context/EthContext';

export default function Mnist() {
  const { isAuthenticated } = useContext(EthContext);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!isAuthenticated) window.location.href = '/';
    setLoading(false);
  }, []);

  if (loading)
    return (
      <div className="w-screen min-h-screen flex items-center justify-center bg-gradient-to-r from-blue-500 to-purple-500">
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
    <div className="w-screen min-h-screen flex items-center justify-center bg-gradient-to-r from-blue-500 to-purple-500">
      <PixelGrid />
    </div>
  );
}
