// Optional asset export only; application builds use the checked-in PNG and ICO files.
const fs = require('node:fs/promises');
const path = require('node:path');
const sharp = require('sharp');

const PNG_SIZES = [16, 24, 32, 48, 64, 128, 256, 512, 1024];
const ICO_SIZES = PNG_SIZES.filter(side => side <= 256);

async function main() {
    sharp.concurrency(2);
    const source = await fs.readFile(path.join(__dirname, 'devcast.svg'));
    const folder = path.join(__dirname, 'png');
    await fs.mkdir(folder, { recursive: true });
    const images = new Map();
    for (const side of PNG_SIZES) {
        const image = await sharp(source, { density: 144 })
            .resize(side, side).ensureAlpha().png({ compressionLevel: 9 }).toBuffer();
        await fs.writeFile(path.join(folder, `devcast-${side}.png`), image);
        images.set(side, image);
    }
    const header = Buffer.alloc(6 + 16 * ICO_SIZES.length);
    header.writeUInt16LE(1, 2);
    header.writeUInt16LE(ICO_SIZES.length, 4);
    let offset = header.length;
    for (const [index, side] of ICO_SIZES.entries()) {
        const at = 6 + 16 * index;
        const image = images.get(side);
        header[at] = side === 256 ? 0 : side;
        header[at + 1] = header[at];
        header.writeUInt16LE(1, at + 4);
        header.writeUInt16LE(32, at + 6);
        header.writeUInt32LE(image.length, at + 8);
        header.writeUInt32LE(offset, at + 12);
        offset += image.length;
    }
    const ico = Buffer.concat([header, ...ICO_SIZES.map(side => images.get(side))]);
    await fs.writeFile(path.join(__dirname, 'devcast.ico'), ico);
    console.log('Готово: 9 PNG RGBA и Windows ICO из branding/devcast.svg.');
}

main().catch(error => {
    console.error(error.message);
    process.exitCode = 1;
});
