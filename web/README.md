# Audio Watermark Web Demo

A web-based demo of the audio watermark encoder/decoder. Record audio, encode messages into it, and decode messages from watermarked audio files.

## Getting Started

### Prerequisites

- Node.js and Yarn
- Rust and wasm-pack (for rebuilding WASM module)

### Install Dependencies

```bash
yarn install
```

### Build WASM Module

The WASM module should already be built in the `pkg` folder. If you need to rebuild it:

```bash
yarn build-wasm
```

Or manually:

```bash
cd ..
wasm-pack build --target web --out-dir pkg
cp -r pkg web/
```

### Development

Start the development server:

```bash
yarn dev
```

The app will be available at `http://localhost:5173`

### Usage

1. **Encode a Message:**
   - Enter your message in the text field
   - Click "Start Recording" to record audio from your microphone
   - Click "Stop Recording" when done
   - Click "Encode & Download WAV" to encode the message and download the watermarked audio file

2. **Decode a Message:**
   - Upload a watermarked WAV file
   - The decoded message will be displayed automatically

### Build

Build for production:

```bash
yarn build
```

The built files will be in the `dist` directory.

### Preview

Preview the production build:

```bash
yarn preview
```

## Tech Stack

- **React 19** - UI library
- **TypeScript** - Type safety
- **Vite** - Build tool and dev server
- **WebAssembly** - Rust code compiled to WASM for browser execution
- **Web Audio API** - Audio recording and processing
- **Yarn** - Package manager



