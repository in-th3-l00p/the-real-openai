import { createBrowserRouter, RouterProvider } from 'react-router-dom'
import './App.css'
import Mnist from './pages/mnist';
import Home from './pages/home';
import Layout from './components/Layout';
import PurchasePage from './pages/purchasePage';
import KnowledgeSharePage from './pages/knowledgeSharePage';
import ChatPage from './pages/chatPage';

const router = createBrowserRouter([
  {
    path: "/",
    element: <Home />
  },
  {
    path: "/mnist",
    element: <Mnist />
  },
  {
    path: "/purchase",
    element: <PurchasePage />
  },
  {
    path: "/knowledge/share",
    element: <KnowledgeSharePage />
  },
  {
    path: "/knowledge",
    element: <ChatPage />
  }
]);

function App() {
  return (
    <Layout>
      <RouterProvider router={router} />
    </Layout>
  )
}

export default App
