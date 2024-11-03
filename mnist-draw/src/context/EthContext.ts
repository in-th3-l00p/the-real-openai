import React, { createContext } from "react";

interface IEthContext {
    isAuthenticated: boolean;
    address: string | null;
    balance: bigint;
    setBalance: React.Dispatch<React.SetStateAction<bigint | null>>;
}

const EthContext = createContext<IEthContext>({} as IEthContext);

export default EthContext;