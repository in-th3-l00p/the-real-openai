// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract AIOwnershipNFT {
    address public owner;

    struct DataSubmission {
        uint256[28][28] data; // Single 28x28 matrix
        uint8 label;          // Corresponding label for the matrix
        bool exists;          // To check if data exists for the user
    }

    mapping(address => DataSubmission) public submissions;
    mapping(address => bool) public hasContributed;
    mapping(address => uint256) public userTokenIds;
    mapping(uint256 => address) public nftOwners;
    mapping(uint256 => string) public tokenURIs;
    mapping(address => uint256) public ownershipPercentages; // Scaled by 10,000
    address[] public contributors;
    uint256 public totalMatrices;
    bool public dataSubmissionOpen = true;
    uint256 public nextTokenId = 1;
    uint256 public lastOwnershipUpdateBlock;

    struct WeightSubmission {
        uint256[] weights;
        uint256 cumulativeOwnership;
        mapping(address => bool) voters;
    }

    mapping(bytes32 => WeightSubmission) private weightSubmissions;
    mapping(address => bool) public hasSubmittedWeights;
    bool public weightsAccepted = false;
    uint256 public acceptableErrorMargin = 1e16; // Adjust as needed

    bytes32[] public weightSubmissionHashes;

    event DataSubmitted(address indexed user);
    event OwnershipCalculated(address indexed user, uint256 ownershipPercentage);
    event NFTMinted(address indexed owner, uint256 tokenId, uint256 ownershipPercentage);
    event TrainingWeightsUpdated(uint256[] weights);

    constructor() {
        owner = msg.sender;
    }

    function submitData(uint256[28][28] memory matrix, uint8 label) public {
        require(dataSubmissionOpen, "Data submission phase is closed");

        if (!hasContributed[msg.sender]) {
            contributors.push(msg.sender);
            hasContributed[msg.sender] = true;
        }

        submissions[msg.sender] = DataSubmission({
            data: matrix,
            label: label,
            exists: true
        });

        totalMatrices++;

        emit DataSubmitted(msg.sender);
    }

    function closeDataSubmissionAndUpdateOwnership() public {
        // uint256 blocksPerTwoWeeks = (2 weeks) / 15; // Approximation for 15-second blocks
        uint256 blocksPerTwoWeeks = 3; //testing
        require(block.number >= lastOwnershipUpdateBlock + blocksPerTwoWeeks, "Ownership can only be updated every two weeks");
        require(dataSubmissionOpen, "Data submission already closed");

        dataSubmissionOpen = false;
        lastOwnershipUpdateBlock = block.number;

        for (uint256 i = 0; i < contributors.length; i++) {
            address user = contributors[i];
            if (submissions[user].exists) {
                uint256 ownershipPercentage = (10000) / totalMatrices; // Each user with a submission has equal ownership
                ownershipPercentages[user] = ownershipPercentage;
                mintOrUpdateOwnershipNFT(user, ownershipPercentage);
                emit OwnershipCalculated(user, ownershipPercentage);
            }
        }

        dataSubmissionOpen = true;
    }

    function mintOrUpdateOwnershipNFT(address to, uint256 ownershipPercentage) internal {
        uint256 tokenId = userTokenIds[to];

        if (tokenId == 0) {
            tokenId = nextTokenId;
            nftOwners[tokenId] = to;
            userTokenIds[to] = tokenId;
            string memory metadata = generateTokenURI(ownershipPercentage);
            tokenURIs[tokenId] = metadata;
            emit NFTMinted(to, tokenId, ownershipPercentage);
            nextTokenId++;
        } else {
            tokenURIs[tokenId] = generateTokenURI(ownershipPercentage);
        }
    }

    function generateTokenURI(uint256 ownershipPercentage) internal pure returns (string memory) {
        string memory imageUrl = "ipfs://QmYWDpWFVq5y4GXixkCQozZ69BDWsYSQz4eTXYhiRvCXDr";
        string memory ownershipStr = uintToDecimalString(ownershipPercentage, 2);

        string memory json = string(abi.encodePacked(
            '{"name":"AI Ownership NFT - ',
            ownershipStr,
            '% Ownership",',
            '"description":"An NFT representing your contribution to AI training data. This NFT signifies ',
            ownershipStr,
            '% ownership of the trained AI model.",',
            '"image":"',
            imageUrl,
            '"}'
        ));

        return string(abi.encodePacked("data:application/json;base64,", encodeBase64(bytes(json))));
    }

    function uintToDecimalString(uint256 value, uint8 decimals) internal pure returns (string memory) {
        uint256 integerPart = value / (10 ** decimals);
        uint256 fractionalPart = value % (10 ** decimals);

        string memory integerStr = uintToString(integerPart);
        string memory fractionalStr = uintToString(fractionalPart);

        while (bytes(fractionalStr).length < decimals) {
            fractionalStr = string(abi.encodePacked("0", fractionalStr));
        }

        return string(abi.encodePacked(integerStr, ".", fractionalStr));
    }

    function uintToString(uint256 _i) internal pure returns (string memory) {
        if (_i == 0) return "0";
        uint256 temp = _i;
        uint256 digits;
        while (temp != 0) {
            digits++;
            temp /= 10;
        }
        bytes memory buffer = new bytes(digits);
        while (_i != 0) {
            buffer[--digits] = bytes1(uint8(48 + _i % 10));
            _i /= 10;
        }
        return string(buffer);
    }

    function encodeBase64(bytes memory data) internal pure returns (string memory) {
        bytes memory base64chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        uint256 datalen = data.length;
        if (datalen == 0) return "";

        uint256 encodedLen = 4 * ((datalen + 2) / 3);
        bytes memory result = new bytes(encodedLen);

        uint256 i = 0;
        uint256 j = 0;

        while (i < datalen) {
            uint256 a0 = uint8(data[i++]);
            uint256 a1 = i < datalen ? uint8(data[i++]) : 0;
            uint256 a2 = i < datalen ? uint8(data[i++]) : 0;

            uint256 n = (a0 << 16) | (a1 << 8) | a2;

            result[j++] = base64chars[(n >> 18) & 63];
            result[j++] = base64chars[(n >> 12) & 63];
            result[j++] = i > datalen + 1 ? bytes1("=") : base64chars[(n >> 6) & 63];
            result[j++] = i > datalen ? bytes1("=") : base64chars[n & 63];
        }

        return string(result);
    }

    function getTokenURI(uint256 tokenId) public view returns (string memory) {
        return tokenURIs[tokenId];
    }

    function submitWeights(uint256[] memory weights) public {
        require(!dataSubmissionOpen, "Data submission must be closed to submit weights");
        require(!hasSubmittedWeights[msg.sender], "User has already submitted weights");
        require(submissions[msg.sender].exists, "Only contributors can submit weights");

        hasSubmittedWeights[msg.sender] = true;

        bytes32 weightHash = keccak256(abi.encodePacked(weights));
        bool foundMatch = false;
        bytes32 matchingHash;

        for (uint256 i = 0; i < weightSubmissionHashes.length; i++) {
            bytes32 existingHash = weightSubmissionHashes[i];
            uint256[] storage existingWeights = weightSubmissions[existingHash].weights;
            if (compareWeightsWithinMargin(existingWeights, weights)) {
                matchingHash = existingHash;
                foundMatch = true;
                break;
            }
        }

        if (!foundMatch) {
            weightSubmissionHashes.push(weightHash);
            WeightSubmission storage newSubmission = weightSubmissions[weightHash];
            for (uint256 i = 0; i < weights.length; i++) {
                newSubmission.weights.push(weights[i]);
            }
            newSubmission.cumulativeOwnership = ownershipPercentages[msg.sender];
            newSubmission.voters[msg.sender] = true;
        } else {
            WeightSubmission storage existingSubmission = weightSubmissions[matchingHash];
            if (!existingSubmission.voters[msg.sender]) {
                existingSubmission.cumulativeOwnership += ownershipPercentages[msg.sender];
                existingSubmission.voters[msg.sender] = true;
            }
        }

        for (uint256 i = 0; i < weightSubmissionHashes.length; i++) {
            bytes32 hash = weightSubmissionHashes[i];
            WeightSubmission storage submission = weightSubmissions[hash];
            if (submission.cumulativeOwnership >= 6500) {
                if (!weightsAccepted) {
                    weightsAccepted = true;
                    emit TrainingWeightsUpdated(submission.weights);
                }
                break;
            }
        }
    }

    function compareWeightsWithinMargin(uint256[] storage weights1, uint256[] memory weights2) internal view returns (bool) {
        if (weights1.length != weights2.length) {
            return false;
        }
        for (uint256 i = 0; i < weights1.length; i++) {
            uint256 diff = weights1[i] > weights2[i] ? weights1[i] - weights2[i] : weights2[i] - weights1[i];
            if (diff > acceptableErrorMargin) {
                return false;
            }
        }
        return true;
    }

    function setAcceptableErrorMargin(uint256 newMargin) public {
        require(msg.sender == owner, "Only owner can set error margin");
        acceptableErrorMargin = newMargin;
    }

    function resetWeightSubmissions() public {
        require(msg.sender == owner, "Only owner can reset weight submissions");
        for (uint256 i = 0; i < weightSubmissionHashes.length; i++) {
            bytes32 hash = weightSubmissionHashes[i];
            delete weightSubmissions[hash];
        }
        delete weightSubmissionHashes;

        for (uint256 i = 0; i < contributors.length; i++) {
            hasSubmittedWeights[contributors[i]] = false;
        }

        weightsAccepted = false;
    }
}