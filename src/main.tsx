import './freeze';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import { applyTheme, findTheme } from './themes';
import './styles.css';

applyTheme(findTheme(localStorage.getItem('lodestone.theme') ?? 'void'));

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
