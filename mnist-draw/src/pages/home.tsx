import { useContext } from "react";
import { ethers } from "ethers";
import EthContext from "../context/EthContext";

export default function Home() {
  const { isAuthenticated } = useContext(EthContext);

  return (
    <>
      {isAuthenticated && (
        <div className="flex flex-col gap-4 items-center">
          <a
            href="/knowledge"
            className="rounded-md shadow-md px-4 py-2 text-white bg-blue-600 hover:bg-blue-800 transition-all"
          >
            Knowledge
          </a>
          <a
            href="/knowledge/share"
            className="rounded-md shadow-md px-4 py-2 text-white bg-blue-600 hover:bg-blue-800 transition-all"
          >
            Knowledge share
          </a>
          <a
            href="/mnist"
            className="rounded-md shadow-md px-4 py-2 text-white bg-blue-600 hover:bg-blue-800 transition-all"
          >
            MNIST
          </a>
          <a
            href="/purchase"
            className="rounded-md shadow-md px-4 py-2 text-white bg-blue-600 hover:bg-blue-800 transition-all"
          >
            Purchase
          </a>
        </div>
        
      )}
      {!isAuthenticated && (
        <button
          type="button"
          className="rounded-md shadow-md px-4 py-2 text-white bg-blue-600 hover:bg-blue-800 transition-all"
          onClick={async () => {
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
          }}
        >
          Authenticate
        </button>
      )}
    </>
  );
}
