import type {
	PublicKeyCredentialCreationOptionsJSON,
	PublicKeyCredentialRequestOptionsJSON,
} from "@simplewebauthn/browser";
import type {
	PasskeyChallenge,
	PasskeySummary,
	SessionInfo,
} from "@/api/generated/model";

// `@simplewebauthn/browser` is imported inside each function so it only loads
// when someone actually uses a Passkey. A static import would put it in the
// public initial bundle (/masuk) and eat into its budget.

export function passkeysSupported(): boolean {
	return typeof window !== "undefined" && "PublicKeyCredential" in window;
}

// The browser (or the person, by dismissing its prompt) ended the ceremony.
// That is not an error worth a toast.
export function isCancellation(error: unknown): boolean {
	return error instanceof Error && error.name === "NotAllowedError";
}

// webauthn-rs answers `{ publicKey, mediation? }`; the library wants the
// `publicKey` part. These are modal ceremonies, so `mediation` is ignored.
function creationOptions(
	challenge: PasskeyChallenge,
): PublicKeyCredentialCreationOptionsJSON {
	return challenge.options.publicKey as PublicKeyCredentialCreationOptionsJSON;
}

function requestOptions(
	challenge: PasskeyChallenge,
): PublicKeyCredentialRequestOptionsJSON {
	return challenge.options.publicKey as PublicKeyCredentialRequestOptionsJSON;
}

async function ceremony<T>(run: () => Promise<T>): Promise<T | null> {
	try {
		return await run();
	} catch (error) {
		if (isCancellation(error)) return null;
		throw error;
	}
}

// Resolves to the new session, or null when the person dismissed the prompt.
export function signInWithPasskey(
	remember: boolean,
): Promise<SessionInfo | null> {
	return ceremony(async () => {
		const [{ startAuthentication }, api] = await Promise.all([
			import("@simplewebauthn/browser"),
			import("@/api/generated/auth/auth"),
		]);
		const challenge = await api.startPasskeySignIn();
		const credential = await startAuthentication({
			optionsJSON: requestOptions(challenge),
		});
		return api.finishPasskeySignIn({
			challenge_id: challenge.challenge_id,
			credential: { ...credential },
			remember,
		});
	});
}

// Resolves to the new Passkey, or null when the person dismissed the prompt.
export function registerPasskey(name: string): Promise<PasskeySummary | null> {
	return ceremony(async () => {
		const [{ startRegistration }, api] = await Promise.all([
			import("@simplewebauthn/browser"),
			import("@/api/generated/passkeys/passkeys"),
		]);
		const challenge = await api.startPasskeyRegistration();
		const credential = await startRegistration({
			optionsJSON: creationOptions(challenge),
		});
		return api.finishPasskeyRegistration({
			challenge_id: challenge.challenge_id,
			name,
			credential: { ...credential },
		});
	});
}

// Resolves true once Sudo Mode is open, false when the person dismissed the
// prompt.
export async function confirmSudoWithPasskey(): Promise<boolean> {
	const confirmed = await ceremony(async () => {
		const [{ startAuthentication }, api] = await Promise.all([
			import("@simplewebauthn/browser"),
			import("@/api/generated/auth/auth"),
		]);
		const challenge = await api.startPasskeySudo();
		const credential = await startAuthentication({
			optionsJSON: requestOptions(challenge),
		});
		await api.finishPasskeySudo({
			challenge_id: challenge.challenge_id,
			credential: { ...credential },
		});
		return true;
	});
	return confirmed ?? false;
}
