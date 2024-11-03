// decapotabila cristina, asa am cumparat-o ca asa a vrut masina
// hey masina, am cumparat cristina,
// hai sa dam o tura sa-ti creasca adrenalina
export const API_URL = "http://127.0.0.1:5000";
export const AUTHORIZATION="0x104f5cc5d1593f1ba2a0eecf5882be85e231aca9";
export const ABI_TALENT=[
  "function purchase() external payable returns (uint256)",
  "function balanceOf(address _address) external view returns (uint256)",
  "function markUsage(address _address) external returns (uint256)"
];

export const KNOWLEDGE_PUBLISH="0xa95799b35a9b71d8793ff8f160ac447b2ccb96f6";
export const KNOWLEDGE_ABI = [
  "function setOwner() external returns (bool)",
  "function isRewardInProgress() external view returns (bool)",
  "function share(string calldata knowledge) external",
  "function getSubmittedKnowledge() external view returns (address[] memory, string[] memory)",
  "function getVote() external view returns (uint256)",
  "function vote(uint256 index) external"
];
