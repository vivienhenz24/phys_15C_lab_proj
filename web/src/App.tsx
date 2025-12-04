import './App.css'
import Encoder from './components/Encoder'
import Decoder from './components/Decoder'

function App() {
  return (
    <div className="app">
      <header className="app-header">
        <h1>Audio Watermark Demo</h1>
        <p>Encode and decode messages in audio using frequency-domain watermarking</p>
      </header>
      <main className="app-main">
        <div className="sections-container">
          <Encoder />
          <Decoder />
        </div>
      </main>
    </div>
  )
}

export default App



