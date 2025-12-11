import { Rnnoise, DenoiseState } from './rnnoise.js';

class RnnoiseProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.frameSize = 480;
    this.inputQueue = [];
    this.outputQueue = [];
    this.denoiseState = null;
    this.ready = false;
    this.frameCount = 0;
    
    console.log(`RnnoiseProcessor: initializing... SampleRate=${globalThis.sampleRate}`);

    Rnnoise.load().then(rnnoise => {
        try {
            this.denoiseState = rnnoise.createDenoiseState();
            this.ready = true;
            console.log("rnnoise active");
        } catch (err) {
            console.error("RnnoiseProcessor: Failed to create denoise state", err);
        }
    }).catch(e => console.error("RnnoiseProcessor: RNNoise load failed", e));
  }

  process(inputs, outputs, parameters) {
    const input = inputs[0];
    const output = outputs[0];
    
    if (!input || !input.length || !output || !output.length) return true;
    
    const inputData = input[0];
    const outputData = output[0];

    if (!this.ready) {
        outputData.set(inputData);
        for (let i = 1; i < output.length; i++) {
            output[i].set(outputData);
        }
        return true;
    }

    try {
        for (let i = 0; i < inputData.length; i++) {
            this.inputQueue.push(inputData[i]);
        }

        while (this.inputQueue.length >= this.frameSize) {
            const frame = new Float32Array(this.inputQueue.splice(0, this.frameSize));
            
            for (let i = 0; i < this.frameSize; i++) {
                frame[i] *= 32768.0;
            }

            this.denoiseState.processFrame(frame);
            
            for (let i = 0; i < this.frameSize; i++) {
                this.outputQueue.push(frame[i] / 32768.0);
            }
        }

        if (this.outputQueue.length >= outputData.length) {
            for (let i = 0; i < outputData.length; i++) {
                outputData[i] = this.outputQueue.shift();
            }
        } else {
            outputData.fill(0);
        }

        for (let i = 1; i < output.length; i++) {
            output[i].set(outputData);
        }
    } catch (e) {
        console.error("RnnoiseProcessor: Error during processing", e);
        this.ready = false;
        outputData.set(inputData);
        for (let i = 1; i < output.length; i++) {
            output[i].set(outputData);
        }
    }

    return true;
  }
}

registerProcessor('rnnoise-processor', RnnoiseProcessor);