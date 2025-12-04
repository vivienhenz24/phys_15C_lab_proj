import './App.css'
import Encoder from './components/Encoder'
import Decoder from './components/Decoder'

function App() {
  return (
    <div className="app">
      <header className="app-header">
        <h1>Audio Watermark Explorer</h1>
        <p>Encode, decode, and visualize watermark bits across time and frequency domains.</p>
      </header>
      <main className="app-main sections-stack">
        <Encoder />
        <Decoder />
      </main>
    </div>
  )
}

export default App


