import {
	createContext,
	type ReactNode,
	useCallback,
	useContext,
	useEffect,
	useMemo,
	useState,
} from "react";
import {
	type AppLocale,
	DEFAULT_LOCALE,
	I18N_NAMESPACES,
	type I18nNamespace,
	SUPPORTED_LOCALES,
} from "@/config/i18n";
import enCommon from "@/config/locales/en/common.json";
import enDialogs from "@/config/locales/en/dialogs.json";
import enEditor from "@/config/locales/en/editor.json";
import enLaunch from "@/config/locales/en/launch.json";
import enSettings from "@/config/locales/en/settings.json";
import enShortcuts from "@/config/locales/en/shortcuts.json";
import enTimeline from "@/config/locales/en/timeline.json";
import ruCommon from "@/config/locales/ru/common.json";
import ruDialogs from "@/config/locales/ru/dialogs.json";
import ruEditor from "@/config/locales/ru/editor.json";
import ruLaunch from "@/config/locales/ru/launch.json";
import ruSettings from "@/config/locales/ru/settings.json";
import ruShortcuts from "@/config/locales/ru/shortcuts.json";
import ruTimeline from "@/config/locales/ru/timeline.json";

const LOCALE_STORAGE_KEY = "devcast.locale";

type LocaleBundle = Record<I18nNamespace, Record<string, unknown>>;

const messages: Record<AppLocale, LocaleBundle> = {
	en: {
		common: enCommon,
		launch: enLaunch,
		editor: enEditor,
		timeline: enTimeline,
		settings: enSettings,
		dialogs: enDialogs,
		shortcuts: enShortcuts,
	},
	ru: {
		common: ruCommon,
		launch: ruLaunch,
		editor: ruEditor,
		timeline: ruTimeline,
		settings: ruSettings,
		dialogs: ruDialogs,
		shortcuts: ruShortcuts,
	},
} as const;

interface I18nContextValue {
	locale: AppLocale;
	setLocale: (locale: AppLocale) => void;
	t: (key: string, fallback?: string, vars?: Record<string, string | number>) => string;
}

const I18nContext = createContext<I18nContextValue | null>(null);

function isSupportedLocale(locale: string): locale is AppLocale {
	return SUPPORTED_LOCALES.includes(locale as AppLocale);
}

function normalizeLocale(locale: string | null | undefined): AppLocale {
	if (!locale) {
		return DEFAULT_LOCALE;
	}

	// Exact match first (e.g. "zh-CN")
	if (isSupportedLocale(locale)) return locale;

	// Canonicalize case (e.g. "zh-cn" → "zh-CN")
	const canonical = SUPPORTED_LOCALES.find((l) => l.toLowerCase() === locale.toLowerCase());
	if (canonical) return canonical;

	// Handle extended subtags like "zh-Hans-CN" → try "zh-CN"
	const parts = locale.split("-");
	if (parts.length >= 3) {
		const langRegion = `${parts[0]}-${parts[parts.length - 1]}`;
		if (isSupportedLocale(langRegion)) {
			return langRegion;
		}
	}

	// Language-only fallback (e.g. "zh" matches "zh-CN")
	const lang = parts[0].toLowerCase();
	const byLang = SUPPORTED_LOCALES.find((l) => l.split("-")[0].toLowerCase() === lang);
	if (byLang) return byLang;

	return DEFAULT_LOCALE;
}

function getInitialLocale(): AppLocale {
	if (typeof window === "undefined") {
		return DEFAULT_LOCALE;
	}

	const storedLocale = window.localStorage.getItem(LOCALE_STORAGE_KEY);
	if (storedLocale) {
		return normalizeLocale(storedLocale);
	}

	return DEFAULT_LOCALE;
}

function getMessageValue(source: unknown, key: string): string | undefined {
	const parts = key.split(".");
	let current: unknown = source;

	for (const part of parts) {
		if (!current || typeof current !== "object" || !(part in current)) {
			return undefined;
		}

		current = (current as Record<string, unknown>)[part];
	}

	return typeof current === "string" ? current : undefined;
}

function interpolate(template: string, vars?: Record<string, string | number>) {
	if (!vars) return template;
	return template.replace(/\{\{\s*([a-zA-Z0-9_]+)\s*\}\}/g, (_match, key) => {
		const value = vars[key];
		return value === undefined ? "" : String(value);
	});
}

function parseKey(key: string): { namespace: I18nNamespace; path: string } {
	const [first, ...rest] = key.split(".");
	if (I18N_NAMESPACES.includes(first as I18nNamespace) && rest.length > 0) {
		return { namespace: first as I18nNamespace, path: rest.join(".") };
	}
	return { namespace: "common", path: key };
}

function translateForLocale(
	locale: AppLocale,
	key: string,
	fallback?: string,
	vars?: Record<string, string | number>,
) {
	const { namespace, path } = parseKey(key);

	const rawValue =
		getMessageValue(messages[locale][namespace], path) ??
		getMessageValue(messages[DEFAULT_LOCALE][namespace], path) ??
		fallback ??
		key;

	return interpolate(rawValue, vars);
}

export function I18nProvider({ children }: { children: ReactNode }) {
	const [locale, setLocaleState] = useState<AppLocale>(getInitialLocale);

	const setLocale = useCallback((nextLocale: AppLocale) => {
		setLocaleState(nextLocale);
		if (typeof window !== "undefined") {
			window.localStorage.setItem(LOCALE_STORAGE_KEY, nextLocale);
		}
	}, []);

	useEffect(() => {
		document.documentElement.lang = locale;
	}, [locale]);

	const t = useCallback(
		(key: string, fallback?: string, vars?: Record<string, string | number>) => {
			return translateForLocale(locale, key, fallback, vars);
		},
		[locale],
	);

	const value = useMemo<I18nContextValue>(
		() => ({
			locale,
			setLocale,
			t,
		}),
		[locale, setLocale, t],
	);

	return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
	const context = useContext(I18nContext);
	if (!context) {
		throw new Error("useI18n must be used within <I18nProvider>");
	}
	return context;
}

export function useScopedT(namespace: I18nNamespace) {
	const { t } = useI18n();
	return useCallback(
		(key: string, fallback?: string, vars?: Record<string, string | number>) => {
			return t(`${namespace}.${key}`, fallback, vars);
		},
		[namespace, t],
	);
}
