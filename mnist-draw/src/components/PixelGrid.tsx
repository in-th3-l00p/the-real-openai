import React, { useRef, useEffect, useState, useContext } from "react";
import predictInput from "../utils/predictInput";
import EthContext from "../context/EthContext";

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

    const canvas = canvasRef.current;
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

    const canvas = canvasRef.current;
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
    if (loading)
        return;
    const canvas = canvasRef.current;
    const ctx = canvas.getContext("2d");

    // Clear the canvas
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    // Draw pixels
    for (let i = 0; i < 28; i++) {
      for (let j = 0; j < 28; j++) {
        ctx.fillStyle = pixels[j][i] === 1 ? "white" : "black";
        ctx.fillRect(i * pixelSize, j * pixelSize, pixelSize, pixelSize);
      }
    }

    // Draw grid lines (optional)
    ctx.strokeStyle = "gray";
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
        <p className="text-white text-xl">Loading...</p>
    )
  return (
    <div className="flex items-center flex-col gap-4">
        {(predict !== undefined) && <p className="text-white text-xl">Prediction: {predict}</p>}
        <div className={"flex gap-8"}>
            <canvas
                ref={canvasRef}
                width={pixelSize * 28}
                height={pixelSize * 28}
                onMouseDown={handleMouseDown}
                onMouseMove={handleMouseMove}
                onMouseUp={handleMouseUp}
                onMouseLeave={handleMouseUp}
                onContextMenu={(e) => e.preventDefault()} // Prevent context menu
                style={{ border: "1px solid black", cursor: "crosshair" }}
            />

            <div className="flex flex-col gap-4">
                <button
                type="button"
                className="bg-red-600 text-white px-4 py-2 rounded-md shadow-md hover:bg-red-700 hover:shadow-xl transition-all"
                onClick={() => {
                    if (confirm("Are you sure you want to reset the matrixx?"))
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
                    className="bg-green-600 text-white px-4 py-2 rounded-md shadow-md hover:bg-green-700 hover:shadow-xl transition-all"
                    disabled={balance === 0n}
                    onClick={() => {
                        setLoading(true);
                        setBalance(balance => (balance ? balance : 1n) - 1n);
                        predictInput(pixels, address!)
                            .then(prediction => setPredict(prediction))
                            .finally(() => setLoading(false));
                    }}
                >
                    Predict
                </button>
            </div>
        </div>
    </div>
  );
};

export default PixelGrid;
