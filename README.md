# The Real One AI
These are the projects of the most decentralized artificial inteligence startup in the world *THE REAL ONE AI*

Our most important project is the implementation of a MNIST classifier using the Solidity programming language, and showing it's output within a web application. Beside the AI model, we developed a smart contract that let users submit training data, as exchange for NFTs.

In the 24 hours of the Arbitrum Stylus Hackathon, the three members of the team: Daniel Selaru, Tyler Valyn Thor, and Tisca Catalin, managed to implement the following projects, as well:
* Web3 MNIST Classifier @ contracts/mnist-classifier/contract.sol, that uses the ai-dao/main.js script to load its weights to the blockchain
* Web3 AI Training DAO @ contracts/ai-dao/Contrat4.sol
* React Frontend for interacting with the calssifier @ sol-classifier
* Stylus MNIST implementation, not functional because of Arbitrum Nitro code size limit @ contracts/mnist-rust (neural network feed forward implemented from scratch)
* Stylus LLM Knowledge Share Contract (permits the uploading of information for a LLM API found @ /mnist_api)
* Stylus Token Authorization for AI API access (ai usage is purchased using this smart contract) @ contracts/api_authorization
* Common AI algorithms api @ gpt_api & mnist_api
* Old frontend that showcases WEB3 and API functionality @ mnist-draw
