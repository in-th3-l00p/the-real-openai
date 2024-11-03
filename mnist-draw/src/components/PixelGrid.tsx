// PixelGrid.jsx
import React, { useRef, useEffect, useState, useContext } from 'react';
import predictInput from '../utils/predictInput';
import EthContext from '../context/EthContext';

const PixelGrid = () => {
  const { address, balance, setBalance } = useContext(EthContext);
  const [pixels, setPixels] = useState(
    Array(28)
      .fill(null)
      .map(() => Array(28).fill(0))
  );
  const [isDrawing, setIsDrawing] = useState(false);
  const [drawValue, setDrawValue] = useState(1); // 1 for fill, 0 for empty
  const [predict, setPredict] = useState<number>();
  const [loading, setLoading] = useState<boolean>(false);

  const canvasRef = useRef(null);
  const pixelSize = 20;

  const handleMouseDown = (e) => {
    e.preventDefault(); // Prevent default behavior
    const x = e.nativeEvent.offsetX;
    const y = e.nativeEvent.offsetY;
    const i = Math.floor(x / pixelSize);
    const j = Math.floor(y / pixelSize);

    if (i >= 0 && i < 28 && j >= 0 && j < 28) {
      const value = e.button === 0 ? 1 : 0; // Left-click fills, right-click empties
      updatePixel(i, j, value);
      setIsDrawing(true);
      setDrawValue(value);
    }
  };

  const handleMouseMove = (e) => {
    if (!isDrawing) return;
    e.preventDefault();
    const x = e.nativeEvent.offsetX;
    const y = e.nativeEvent.offsetY;
    const i = Math.floor(x / pixelSize);
    const j = Math.floor(y / pixelSize);

    if (i >= 0 && i < 28 && j >= 0 && j < 28) {
      updatePixel(i, j, drawValue);
    }
  };

  const handleMouseUp = () => {
    setIsDrawing(false);
  };

  const updatePixel = (i, j, value) => {
    setPixels((prevPixels) => {
      const newPixels = prevPixels.map((row) => row.slice());
      newPixels[j][i] = value;
      return newPixels;
    });
  };

  useEffect(() => {
    if (loading) return;
    const canvas = canvasRef.current;
    const ctx = canvas.getContext('2d');

    // Clear the canvas
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    // Draw pixels
    for (let i = 0; i < 28; i++) {
      for (let j = 0; j < 28; j++) {
        ctx.fillStyle = pixels[j][i] === 1 ? '#ffffff' : '#000000';
        ctx.fillRect(i * pixelSize, j * pixelSize, pixelSize, pixelSize);
      }
    }

    // Draw grid lines (optional)
    ctx.strokeStyle = '#2d2d2d';
    for (let i = 0; i <= 28; i++) {
      ctx.beginPath();
      ctx.moveTo(i * pixelSize, 0);
      ctx.lineTo(i * pixelSize, 28 * pixelSize);
      ctx.stroke();

      ctx.beginPath();
      ctx.moveTo(0, i * pixelSize);
      ctx.lineTo(28 * pixelSize, i * pixelSize);
      ctx.stroke();
    }
  }, [pixels, loading]);

  if (loading)
    return (
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
        <p className="text-white text-2xl mt-4">Processing...</p>
      </div>
    );

  return (
    <div className="flex flex-col items-center gap-6">
      {predict !== undefined && (
        <div className="bg-white bg-opacity-80 rounded-lg px-6 py-4 shadow-md">
          <p className="text-2xl font-bold text-gray-800">
            Prediction: {predict}
          </p>
        </div>
      )}
      <div className="flex flex-col md:flex-row items-center gap-8">
        <canvas
          ref={canvasRef}
          width={pixelSize * 28}
          height={pixelSize * 28}
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
          onMouseLeave={handleMouseUp}
          onContextMenu={(e) => e.preventDefault()} // Prevent context menu
          className="border-4 border-gray-800 rounded-md shadow-lg cursor-crosshair"
        />
        <div className="flex flex-col gap-4">
          <button
            type="button"
            className="bg-red-500 text-white px-6 py-3 rounded-md shadow-md hover:bg-red-600 focus:outline-none focus:ring-2 focus:ring-red-400 transition-all"
            onClick={() => {
              if (
                window.confirm(
                  'Are you sure you want to reset the drawing?'
                )
              )
                setPixels(
                  Array(28)
                    .fill(null)
                    .map(() => Array(28).fill(0))
                );
            }}
          >
            Reset
          </button>
          <button
            type="button"
            className="bg-green-500 text-white px-6 py-3 rounded-md shadow-md hover:bg-green-600 focus:outline-none focus:ring-2 focus:ring-green-400 transition-all disabled:opacity-50"
            disabled={balance === 0n || loading}
            onClick={() => {
              setLoading(true);
              setBalance((balance) => (balance ? balance : 1n) - 1n);
              predictInput(pixels, address!)
                .then((prediction) => setPredict(prediction))
                .finally(() => setLoading(false));
            }}
          >
            {loading ? 'Predicting...' : 'Predict'}
          </button>
        </div>
      </div>
    </div>
  );
};

export default PixelGrid;
