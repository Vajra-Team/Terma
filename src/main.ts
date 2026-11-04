import './styles/vajra/styles.css';
import './styles/components.css';
import './styles/app.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { T } from './lib/texte';

document.documentElement.lang = T.sprache;
mount(App, { target: document.getElementById('app')! });
