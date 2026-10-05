import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
// Keep this list in sync with the editable runtime icon assets.  An ICO does
// not need every possible size: these five cover Explorer, shortcuts and the
// Windows shell without requiring separate hand-maintained .ico files.
const iconSizes = [16, 32, 64, 128, 256];
const sourceDirectory = path.join(projectRoot, "public", "content", "icons");
const targetPath = path.join(projectRoot, "branding", "app-icon", "win", "icon.ico");

const entries = iconSizes.map((size) => {
	const sourcePath = path.join(sourceDirectory, `devcast-${size}.png`);
	if (!fs.existsSync(sourcePath)) {
		throw new Error(`Missing Windows icon source: ${sourcePath}`);
	}
	return { size, data: fs.readFileSync(sourcePath) };
});

const headerSize = 6 + entries.length * 16;
let offset = headerSize;
const header = Buffer.alloc(headerSize);
header.writeUInt16LE(0, 0); // reserved
header.writeUInt16LE(1, 2); // ICO type
header.writeUInt16LE(entries.length, 4);

for (const [index, entry] of entries.entries()) {
	const entryOffset = 6 + index * 16;
	header.writeUInt8(entry.size === 256 ? 0 : entry.size, entryOffset);
	header.writeUInt8(entry.size === 256 ? 0 : entry.size, entryOffset + 1);
	header.writeUInt8(0, entryOffset + 2);
	header.writeUInt8(0, entryOffset + 3);
	header.writeUInt16LE(1, entryOffset + 4);
	header.writeUInt16LE(32, entryOffset + 6);
	header.writeUInt32LE(entry.data.length, entryOffset + 8);
	header.writeUInt32LE(offset, entryOffset + 12);
	offset += entry.data.length;
}

fs.mkdirSync(path.dirname(targetPath), { recursive: true });
fs.writeFileSync(targetPath, Buffer.concat([header, ...entries.map((entry) => entry.data)]));
console.log(`[build-windows-app-icon] Generated ${targetPath} from ${entries.length} PNG sizes.`);
