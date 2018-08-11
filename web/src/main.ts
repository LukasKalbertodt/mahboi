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
    let color = 0;
    window.setInterval(() => {
        let pixelOffset = y * (width * 4) + x * 4;
        imageData.data[pixelOffset] = color % 256;
        imageData.data[pixelOffset + 1] = (color / 256) % 256;
        imageData.data[pixelOffset + 2] = 100;

        color += 16;

        // Advance to next position
        x++;
        if (x == width) {
            x = 0;
            y++;
            if (y == height) {
                y = 0;
            }
        }
    }, 2);

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
document.addEventListener("DOMContentLoaded", init);
