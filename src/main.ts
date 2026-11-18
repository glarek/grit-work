import '@fontsource-variable/figtree/wght.css';
import '@fontsource-variable/jetbrains-mono/wght.css';
import './app.css';
import App from './App.svelte';

const target = document.getElementById('app');

if (!target) {
  throw new Error('找不到应用挂载节点 #app');
}

const app = new App({ target });

export default app;
