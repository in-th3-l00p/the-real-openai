import { BrowserProvider, Contract } from "ethers";
import { ABI_TALENT, API_URL, AUTHORIZATION } from "./contracts";

// Define API URL and ABI

// Function to mark usage and fetch prediction
export default async function predictInput(input: number[][], address: string): Promise<number> {
  if (!window.ethereum) {
    throw new Error("Ethereum wallet is not available.");
  }

  // Set up provider and signer
  const provider = new BrowserProvider(window.ethereum);
  const signer = await provider.getSigner();

  // Initialize contract instance
  const contract = new Contract(AUTHORIZATION, ABI_TALENT, signer);

  // Execute markUsage before fetching
  await contract.markUsage(address);

  // Fetch prediction from API
  const response = await fetch(`${API_URL}/predict`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json"
    },
    body: JSON.stringify(input)
  });

  // Parse and return the prediction from the response
  const data = await response.json();
  return data["prediction"];
}
