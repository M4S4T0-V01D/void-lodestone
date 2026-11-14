import './freeze';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import { applyTheme, findTheme, withAccent } from './themes';
import './styles.css';

applyTheme(withAccent(findTheme(localStorage.getItem('lodestone.theme') ?? 'void'), localStorage.getItem('lodestone.accent') ?? ''));

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
