// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

contract AIOwnershipNFT {
    address public owner;

    struct DataSubmission {
        uint256[28][28] data; // Single 28x28 matrix
        uint8 label;          // Corresponding label for the matrix
        bool exists;          // To check if a submission exists for an address
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

    // ERC721 Storage
    mapping(uint256 => address) private _tokenApprovals;
    mapping(address => mapping(address => bool)) private _operatorApprovals;

    // Events
    event DataSubmitted(address indexed user, uint256 tokenId);
    event OwnershipCalculated(address indexed user, uint256 ownershipPercentage);
    event NFTMinted(address indexed owner, uint256 tokenId, uint256 ownershipPercentage);
    event Transfer(address indexed from, address indexed to, uint256 indexed tokenId);
    event Approval(address indexed owner, address indexed approved, uint256 indexed tokenId);
    event ApprovalForAll(address indexed owner, address indexed operator, bool approved);

    constructor() {
        owner = msg.sender;
    }

    function submitData(uint256[28][28] memory data, uint8 label) public {
        require(dataSubmissionOpen, "Data submission phase is closed");

        for (uint256 i = 0; i < 28; i++) {
            require(data[i].length == 28, "Each row must be 28 elements long");
        }

        submissions[msg.sender] = DataSubmission({
            data: data,
            label: label,
            exists: true
        });

        if (!hasContributed[msg.sender]) {
            contributors.push(msg.sender);
            hasContributed[msg.sender] = true;
        }

        totalMatrices += 1;

        emit DataSubmitted(msg.sender, nextTokenId);
    }

    function closeDataSubmissionAndUpdateOwnership() public {
      //  uint256 blocksPerTwoWeeks = (2 weeks) / 15; // Approximation for 15-second blocks
        uint256 blocksPerTwoWeeks = 4;
        require(block.number >= lastOwnershipUpdateBlock + blocksPerTwoWeeks, "Ownership can only be updated every two weeks");
        require(dataSubmissionOpen, "Data submission already closed");

        dataSubmissionOpen = false;
        lastOwnershipUpdateBlock = block.number;

        for (uint256 i = 0; i < contributors.length; i++) {
            address user = contributors[i];

            if (submissions[user].exists) {
                uint256 ownershipPercentage = (10000) / totalMatrices;
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
            emit Transfer(address(0), to, tokenId);
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

    // ERC721 Functions

    function balanceOf(address _owner) public view returns (uint256) {
        require(_owner != address(0), "Invalid address");
        uint256 count = 0;
        for (uint256 i = 1; i < nextTokenId; i++) {
            if (nftOwners[i] == _owner) {
                count++;
            }
        }
        return count;
    }

    function ownerOf(uint256 tokenId) public view returns (address) {
        address ownerAddress = nftOwners[tokenId];
        require(ownerAddress != address(0), "Token does not exist");
        return ownerAddress;
    }

    function approve(address to, uint256 tokenId) public {
        address ownerAddress = ownerOf(tokenId);
        require(to != ownerAddress, "Cannot approve to current owner");
        require(
            msg.sender == ownerAddress || isApprovedForAll(ownerAddress, msg.sender),
            "Not authorized to approve"
        );

        _tokenApprovals[tokenId] = to;
        emit Approval(ownerAddress, to, tokenId);
    }

    function getApproved(uint256 tokenId) public view returns (address) {
        require(nftOwners[tokenId] != address(0), "Token does not exist");
        return _tokenApprovals[tokenId];
    }

    function setApprovalForAll(address operator, bool approved) public {
        require(operator != msg.sender, "Cannot approve to yourself");
        _operatorApprovals[msg.sender][operator] = approved;
        emit ApprovalForAll(msg.sender, operator, approved);
    }

    function isApprovedForAll(address ownerAddress, address operator) public view returns (bool) {
        return _operatorApprovals[ownerAddress][operator];
    }

    function transferFrom(address from, address to, uint256 tokenId) public {
        address ownerAddress = ownerOf(tokenId);
        require(
            msg.sender == ownerAddress ||
            getApproved(tokenId) == msg.sender ||
            isApprovedForAll(ownerAddress, msg.sender),
            "Not authorized to transfer"
        );
        require(ownerAddress == from, "Transfer from incorrect owner");
        require(to != address(0), "Transfer to zero address");

        _transfer(from, to, tokenId);
    }

    function _transfer(address from, address to, uint256 tokenId) internal {
        nftOwners[tokenId] = to;
        emit Transfer(from, to, tokenId);
    }
}