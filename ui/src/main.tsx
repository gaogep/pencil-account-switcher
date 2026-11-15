import { render } from 'preact';
import { App } from './App';
import './theme.css';
import './components/components.css';
import './screens.css';
render(<App />, document.getElementById('app')!);
