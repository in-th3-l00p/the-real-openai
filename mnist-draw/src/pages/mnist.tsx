import { useContext, useEffect, useState } from 'react';
import PixelGrid from '../components/PixelGrid'
import EthContext from '../context/EthContext';

export default function Mnist() {
    const { isAuthenticated } = useContext(EthContext);
    const [ loading, setLoading ] = useState(true);
    useEffect(() => {
        if (!isAuthenticated)
            window.location.href = "/";
        setLoading(false);
    }, []);

    if (loading)
        return (
            <p className="text-white text-xl">Loading...</p>
        )
    return (
        <PixelGrid />
    );
}