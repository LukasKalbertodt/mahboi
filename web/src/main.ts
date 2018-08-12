// import * as mahboi from './mahboi_web';

// declare namespace mahboi_web;
// const mahboi = mahboi_web;
// declare namespace mahboi {
//     function makeGreeting(s: string): string;
// };

// declare mahboi_web;

import mahboi = wasm_bindgen;



const load_wasm = () => {
    wasm_bindgen("/mahboi_web_bg.wasm").then(() => {
        console.log(mahboi.get_color(0, 100));
        init()
    });
}

const init = () => {
    let x = 0;
    let y = 0;

    let canvas = getCanvas();
    let ctx = canvas.getContext("2d", { alpha: false })!;

    const width = canvas.width;
    const height = canvas.height;
    let imageData = new ImageData(width, height);

    // Set all pixels to fully opaque
    for (let offset = 3; offset < imageData.data.length; offset += 4) {
        imageData.data[offset] = 255;
    }

    // Update pixels to rainbow colors (dummy code, can be removed later)
    window.setInterval(() => {
        for (let i = 0; i < 10; i++) {
            let color = mahboi.get_color(x, y);

            let offset = y * (width * 4) + x * 4;
            imageData.data[offset + 0] = color.r;
            imageData.data[offset + 1] = color.g;
            imageData.data[offset + 2] = color.b;

            // Advance to next position
            x++;
            if (x == width) {
                x = 0;
                y++;
                if (y == height) {
                    y = 0;
                }
            }
        }
    }, 10);

    // Update the canvas with our image data once a frame
    repeatEveryFrame(() => {
        ctx.putImageData(imageData, 0, 0);
    });
}

// Executes the given function once every frame (using `requestAnimationFrame`).
const repeatEveryFrame = (fn: () => void) => {
    const repeat = () => {
        fn();
        window.requestAnimationFrame(repeat);
    };
    repeat();
}

// Returns the canvas representing the main screen
const getCanvas = (): HTMLCanvasElement => {
    return <HTMLCanvasElement> document.getElementById('main-screen')!;
}


// Once the DOM is ready, initialize everything
document.addEventListener("DOMContentLoaded", load_wasm);
