import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const MAX_ATTEMPTS = 4;
const RETRY_DELAY_MS = 750;

function wait(delayMs) {
	return new Promise((resolve) => setTimeout(resolve, delayMs));
}

function runViteBuild() {
	return new Promise((resolve) => {
		const viteCliPath = path.resolve(
			path.dirname(fileURLToPath(import.meta.url)),
			"../node_modules/vite/bin/vite.js",
		);
		const child = spawn(process.execPath, [
			viteCliPath,
			"build",
			"--config",
			"vite.config.ts",
		], {
			cwd: process.cwd(),
			stdio: ["inherit", "inherit", "pipe"],
		});

		let stderr = "";
		child.stderr.setEncoding("utf8");
		child.stderr.on("data", (chunk) => {
			stderr += chunk;
			process.stderr.write(chunk);
		});
		child.on("close", (code) => resolve({ code: code ?? 1, stderr }));
	});
}

function isRetryableElectronBundleLock(stderr) {
	return (
		/(UNKNOWN|EBUSY|EPERM):.*(?:open|rename|unlink)/i.test(stderr) &&
		/dist-electron[\\/]main\.cjs/i.test(stderr)
	);
}

for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt += 1) {
	const result = await runViteBuild();
	if (result.code === 0) {
		process.exit(0);
	}

	if (!isRetryableElectronBundleLock(result.stderr) || attempt === MAX_ATTEMPTS) {
		process.exit(result.code);
	}

	console.warn(
		`[build:vite] dist-electron/main.cjs is temporarily locked; retrying ${attempt}/${MAX_ATTEMPTS} in ${RETRY_DELAY_MS}ms...`,
	);
	await wait(RETRY_DELAY_MS);
}
