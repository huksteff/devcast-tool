import { afterEach, expect, it, vi } from "vitest";
const native = vi.hoisted(() =>
	Object.assign(
		vi.fn(() => "toast-key"),
		{
			success: vi.fn(),
			danger: vi.fn(),
			update: vi.fn(() => "toast-key"),
			clear: vi.fn(),
			close: vi.fn(),
		},
	),
);
vi.mock("@heroui/react", () => ({ Toast: { Provider: () => null }, toast: native }));
import { toast } from "./toast";
afterEach(() => {
	toast.dismiss();
	vi.clearAllMocks();
	vi.unstubAllGlobals();
});
it("copies the error title and details through the native action", async () => {
	const writeText = vi.fn().mockResolvedValue(undefined);
	vi.stubGlobal("navigator", { clipboard: { writeText } });
	toast.error("Export failed", { description: "Encoder unavailable" });
	const options = (
		native.mock.calls as unknown as [
			string,
			{ variant: string; actionProps: { children: string; onPress: () => void } },
		][]
	)[0][1];
	expect(options.variant).toBe("danger");
	expect(options.actionProps.children).toBe("Copy");
	options.actionProps.onPress();
	await vi.waitFor(() => expect(native.success).toHaveBeenCalledWith("Error copied"));
	expect(writeText).toHaveBeenCalledWith("Export failed\n\nEncoder unavailable");
});
it("preserves explicit actions and updates named notifications", () => {
	const onClick = vi.fn();
	toast.error("Retry", { id: "job", action: { label: "Retry", onClick } });
	toast.success("Done", { id: "job" });
	expect(native.update).toHaveBeenCalledWith(
		"toast-key",
		"Done",
		expect.objectContaining({ variant: "success" }),
	);
	expect(native).toHaveBeenCalledWith(
		expect.objectContaining({ props: expect.objectContaining({ children: "Retry" }) }),
		expect.objectContaining({ actionProps: { children: "Retry", onPress: onClick } }),
	);
});
it("uses the Electron clipboard when browser clipboard permissions are unavailable", async () => {
	const copyTextToClipboard = vi.fn().mockResolvedValue({ success: true });
	vi.stubGlobal("window", { electronAPI: { copyTextToClipboard } });
	vi.stubGlobal("navigator", {});
	toast.error("Model missing", { description: "C:\\Whisper\\small.bin" });
	const options = (native.mock.calls as unknown as [unknown, { actionProps: { onPress: () => void } }][])[0][1];
	options.actionProps.onPress();
	await vi.waitFor(() => expect(native.success).toHaveBeenCalledWith("Error copied"));
	expect(copyTextToClipboard).toHaveBeenCalledWith("Model missing\n\nC:\\Whisper\\small.bin");
});
it("keeps failures selectable without recursively adding failed Copy actions", async () => {
	vi.stubGlobal("window", { electronAPI: { copyTextToClipboard: vi.fn().mockResolvedValue({ success: false }) } });
	toast.error("Model missing");
	const options = (native.mock.calls as unknown as [unknown, { actionProps: { onPress: () => void } }][])[0][1];
	options.actionProps.onPress();
	await vi.waitFor(() => expect(native.danger).toHaveBeenCalledOnce());
	expect(native.danger.mock.calls[0][1]).toEqual(expect.objectContaining({
		timeout: 0, description: expect.objectContaining({ props: expect.objectContaining({ className: expect.stringContaining("select-text"), children: "Model missing" }) }),
	}));
});
