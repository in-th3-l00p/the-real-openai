// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract AIOwnershipNFT {
    address public owner;

    struct DataSubmission {
        uint256[][][] data; // Array of 28x28 matrices
        uint8[] labels;     // Corresponding labels for each matrix
        uint256 numberOfMatrices;
    }

    mapping(address => DataSubmission) public submissions;
    mapping(address => bool) public hasContributed;
    mapping(address => uint256) public userTokenIds;
    mapping(uint256 => address) public nftOwners;
    mapping(uint256 => string) public tokenURIs; // URI now points to external storage
    mapping(address => uint256) public ownershipPercentages; // Scaled by 10,000
    address[] public contributors;
    uint256 public totalMatrices;
    bool public dataSubmissionOpen = true;
    uint256 public nextTokenId = 1;
    uint256 public lastOwnershipUpdateBlock;

    // Events
    event DataSubmitted(address indexed user, uint256 matricesSubmitted);
    event OwnershipCalculated(address indexed user, uint256 ownershipPercentage);
    event NFTMinted(address indexed owner, uint256 tokenId, string metadataURI);

    constructor() {
        owner = msg.sender;
    }

    // Function to submit data and labels
    function submitData(uint256[][][] memory data, uint8[] memory labels) public {
        require(dataSubmissionOpen, "Data submission phase is closed");
        require(data.length > 0 && data.length == labels.length, "Data and labels length must match");

        uint256 matricesSubmitted = data.length;

        // Validate each matrix
        for (uint256 i = 0; i < matricesSubmitted; i++) {
            require(data[i].length == 28, "Each data matrix must be 28x28");
            for (uint256 j = 0; j < 28; j++) {
                require(data[i][j].length == 28, "Each data matrix must be 28x28");
            }
        }

        // Check if the user is contributing for the first time
        if (!hasContributed[msg.sender]) {
            contributors.push(msg.sender);
            hasContributed[msg.sender] = true;
        }

        // Append data and labels to the user's existing submission
        for (uint256 i = 0; i < matricesSubmitted; i++) {
            submissions[msg.sender].data.push(data[i]);
            submissions[msg.sender].labels.push(labels[i]);
        }

        submissions[msg.sender].numberOfMatrices += matricesSubmitted;
        totalMatrices += matricesSubmitted;

        emit DataSubmitted(msg.sender, matricesSubmitted);
    }

    // Function to close data submission and update ownership percentages
    function closeDataSubmissionAndUpdateOwnership() public {
        // uint256 blocksPerTwoWeeks = (2 weeks) / 15; // Assuming 15-second blocks
        uint256 blocksPerTwoWeeks = 3; // for testing purposes
        require(block.number >= lastOwnershipUpdateBlock + blocksPerTwoWeeks, "Ownership can only be updated every two weeks");
        require(dataSubmissionOpen, "Data submission already closed");

        dataSubmissionOpen = false;
        lastOwnershipUpdateBlock = block.number;

        // Update NFTs based on the updated ownership percentages
        for (uint256 i = 0; i < contributors.length; i++) {
            address user = contributors[i];
            uint256 userMatrices = submissions[user].numberOfMatrices;

            if (userMatrices > 0) {
                uint256 ownershipPercentage = (userMatrices * 10000) / totalMatrices;
                ownershipPercentages[user] = ownershipPercentage;
                mintOrUpdateOwnershipNFT(user, ""); // Metadata URI will be updated off-chain
                emit OwnershipCalculated(user, ownershipPercentage);
            }
        }

        dataSubmissionOpen = true; // Reopen data submission for the next round
    }

    // Function to mint a new NFT or update existing NFT metadata URI
    function mintOrUpdateOwnershipNFT(address to, string memory metadataURI) internal {
        uint256 tokenId = userTokenIds[to];

        if (tokenId == 0) {
            // Mint new NFT
            tokenId = nextTokenId;
            nftOwners[tokenId] = to;
            userTokenIds[to] = tokenId;
            tokenURIs[tokenId] = metadataURI;
            emit NFTMinted(to, tokenId, metadataURI);
            nextTokenId++;
        } else {
            // Update metadata for existing NFT
            tokenURIs[tokenId] = metadataURI;
        }
    }

    // Function to retrieve token URI
    function getTokenURI(uint256 tokenId) public view returns (string memory) {
        return tokenURIs[tokenId];
    }

    // Function to set ownership percentages (onlyOwner)
    function setOwnershipPercentage(address user, uint256 percentage) public {
        require(msg.sender == owner, "Only owner can set ownership percentage");
        ownershipPercentages[user] = percentage;
    }

    // Function to set token URI (onlyOwner)
    function setTokenURI(uint256 tokenId, string memory metadataURI) public {
        require(msg.sender == owner, "Only owner can set token URI");
        tokenURIs[tokenId] = metadataURI;
    }

    function getSubmission(address user) public view returns (uint256[][][] memory, uint8[] memory, uint256) {
        return (submissions[user].data, submissions[user].labels, submissions[user].numberOfMatrices);
    }
}