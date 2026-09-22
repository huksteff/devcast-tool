import { afterEach, beforeEach, expect, it, vi } from "vitest";

const exchange = vi.hoisted(() => vi.fn(async (_code: string) => ({ error: null })));
vi.mock("@supabase/supabase-js", () => ({
	createClient: () => ({ auth: { exchangeCodeForSession: exchange } }),
}));
beforeEach(() => {
	vi.resetModules();
	exchange.mockClear();
	vi.stubEnv("VITE_SUPABASE_URL", "https://auth.example.test");
	vi.stubEnv("VITE_SUPABASE_PUBLISHABLE_KEY", "test-key");
});
afterEach(() => vi.unstubAllEnvs());

it("exchanges a callback once when live and pending delivery overlap", async () => {
	const { completeAuthCallback } = await import("./recordlyAuth");
	const url = "recordly://auth/callback?code=one-time-code";
	await Promise.all([completeAuthCallback(url), completeAuthCallback(url)]);
	await completeAuthCallback(url);
	expect(exchange).toHaveBeenCalledExactlyOnceWith("one-time-code");
});

it("shows provider errors without attempting a code exchange", async () => {
	const { completeAuthCallback } = await import("./recordlyAuth");
	await expect(
		completeAuthCallback(
			"recordly://auth/callback?error=denied&error_description=Sign-in+cancelled",
		),
	).rejects.toThrow("Sign-in cancelled");
	expect(exchange).not.toHaveBeenCalled();
});
