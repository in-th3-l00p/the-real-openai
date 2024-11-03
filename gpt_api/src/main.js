import express from 'express';
import { JsonRpcProvider, Wallet, isAddress, Contract } from 'ethers';
import OpenAI from 'openai';
import dotenv from 'dotenv';
import cors from "cors";

dotenv.config();

const { OPENAI_API_KEY, PRIVATE_KEY, RPC_URL, PORT } = process.env;

if (!OPENAI_API_KEY || !PRIVATE_KEY || !RPC_URL) {
  console.error('Error: Missing required environment variables.');
  process.exit(1);
}

const CONTRACT_ADDRESS = '0x104f5cc5d1593f1ba2a0eecf5882be85e231aca9';
const ABI = [
  "function purchase() external payable returns (uint256)",
  "function balanceOf(address _address) external view returns (uint256)",
  "function markUsage(address _address) external returns (uint256)"
];

const generalKnowledgeSentences = [
  "The Earth revolves around the Sun.",
  "Water boils at 100 degrees Celsius.",
  "Light travels faster than sound.",
  "The human body has 206 bones."
];

// -----------------------------
// Initialize Express App
// -----------------------------

const app = express();
app.use(express.json());
app.use(cors());

// -----------------------------
// Initialize OpenAI
// -----------------------------

const openai = new OpenAI({
  apiKey: OPENAI_API_KEY,
});

// -----------------------------
// Initialize Ethers.js Components
// -----------------------------

const provider = new JsonRpcProvider(RPC_URL);
const wallet = new Wallet(PRIVATE_KEY, provider);
const contract = new Contract(CONTRACT_ADDRESS, ABI, wallet);

app.post('/query-ai', async (req, res) => {
  try {
    const { query, ethAddress } = req.body;

    if (!query || !ethAddress) {
      return res.status(400).json({ error: 'Missing query or ethAddress in request body.' });
    }

    if (!isAddress(ethAddress)) {
      return res.status(400).json({ error: 'Invalid Ethereum address.' });
    }

    const balance = await contract.balanceOf(ethAddress);

    if (balance < 1n) {
      return res.status(403).json({ error: 'Insufficient credits.' });
    }

    const prompt = `${generalKnowledgeSentences.join(' ')}\n\n${query}`;

    const completion = await openai.chat.completions.create({
      model: "gpt-4",
      messages: [
        { role: "system", content: "You are a knowledgeable assistant." },
        { role: "user", content: prompt }
      ],
      temperature: 0.7,
      max_tokens: 150 
    });

    const aiResponse = completion.choices[0].message.content.trim();

    // Optional: Mark Usage (State-Changing Operation)
    try {
      const markUsageTx = await contract.markUsage(ethAddress);
      await markUsageTx.wait(); // Wait for the transaction to be mined
      console.log(`Marked usage for address: ${ethAddress}`);
    } catch (txError) {
      console.error(`Failed to mark usage for address ${ethAddress}:`, txError);
      // Depending on requirements, you might want to handle this differently
    }

    // Return AI Response
    res.json({ result: aiResponse });
  } catch (error) {
    console.error('Error:', error);
    res.status(500).json({ error: 'Internal server error.' });
  }
});

// -----------------------------
// Start the Server
// -----------------------------

const serverPort = PORT || 3000;
app.listen(serverPort, () => {
  console.log(`Server running on port ${serverPort}`);
});
