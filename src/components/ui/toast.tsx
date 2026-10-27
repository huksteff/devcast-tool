import { Toast, toast as heroToast } from "@heroui/react";
import { isValidElement, type ReactNode } from "react";

type Options = {
	id?: string | number;
	description?: ReactNode;
	duration?: number;
	action?: { label: ReactNode; onClick: () => void };
	onDismiss?: () => void;
};
type Variant = "default" | "accent" | "success" | "warning" | "danger";
const ids = new Map<string | number, string>();

async function copyError(text: string) {
	try {
		if (typeof window !== "undefined" && window.electronAPI?.copyTextToClipboard) {
			const result = await window.electronAPI.copyTextToClipboard(text);
			if (!result.success) throw new Error(result.error || "Clipboard unavailable");
		} else {
			await navigator.clipboard.writeText(text);
		}
		heroToast.success("Error copied");
	} catch {
		heroToast.danger("Could not copy error. Select the error text and press Ctrl+C.", {
			description: <span className="select-text whitespace-pre-wrap break-words">{text}</span>,
			timeout: 0,
		});
	}
}

function plainText(value: ReactNode): string {
	if (typeof value === "string" || typeof value === "number") return String(value);
	if (Array.isArray(value)) return value.map(plainText).join("");
	if (isValidElement<{ children?: ReactNode }>(value)) return plainText(value.props.children);
	return "";
}

// Keep existing callers compatible while HeroUI owns the queue and presentation.
function notify(title: ReactNode, options: Options = {}, variant: Variant = "default") {
	const errorText = [plainText(title), plainText(options.description)]
		.filter(Boolean)
		.join("\n\n");
	const action = options.action;
	const nativeOptions = {
		variant,
		description: variant === "danger" && options.description
			? <span className="select-text whitespace-pre-wrap break-words">{options.description}</span>
			: options.description,
		timeout:
			options.duration === Infinity
				? 0
				: (options.duration ?? (variant === "danger" ? 8000 : 4000)),
		onClose: () => {
			if (options.id !== undefined) ids.delete(options.id);
			options.onDismiss?.();
		},
		actionProps: action
			? {
					children: action.label,
					onPress: action.onClick,
				}
			: variant === "danger" && errorText
				? {
						children: "Copy",
						onPress: () => { void copyError(errorText); },
					}
				: undefined,
	};
	const previous = options.id === undefined ? undefined : ids.get(options.id);
	const displayTitle = variant === "danger"
		? <span className="select-text whitespace-pre-wrap break-words">{title}</span> : title;
	const key = previous
		? heroToast.update(previous, displayTitle, nativeOptions)
		: heroToast(displayTitle, nativeOptions);
	if (options.id !== undefined) ids.set(options.id, key);
	return key;
}
export const toast = Object.assign(notify, {
	success: (message: ReactNode, options?: Options) => notify(message, options, "success"),
	error: (message: ReactNode, options?: Options) => notify(message, options, "danger"),
	info: (message: ReactNode, options?: Options) => notify(message, options, "accent"),
	warning: (message: ReactNode, options?: Options) => notify(message, options, "warning"),
	dismiss: (id?: string | number) => {
		if (id === undefined) {
			heroToast.clear();
			ids.clear();
		} else heroToast.close(ids.get(id) ?? String(id));
	},
});
export function Toaster({ className }: { className?: string }) {
	return <Toast.Provider placement="bottom end" className={className} />;
}
