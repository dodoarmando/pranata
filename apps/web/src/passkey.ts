// The browser half of a passkey ceremony: the API's options in, the credential's JSON out.
// Both reject when the User cancels, the browser finds no passkey or cannot parse JSON options.

// The options travel as a JSON string: server functions only carry values typed as serializable.
type Options<T> = { publicKey: T };

// The API takes `toJSON()` as it is; the DOM's interfaces only lack the index signature.
const toJson = (credential: Credential | null) =>
  (credential as PublicKeyCredential).toJSON() as unknown as Record<string, unknown>;

/** Creates a passkey from the API's registration options. */
export async function createPasskeyCredential(options: string) {
  const { publicKey }: Options<PublicKeyCredentialCreationOptionsJSON> = JSON.parse(options);
  return toJson(
    await navigator.credentials.create({
      publicKey: PublicKeyCredential.parseCreationOptionsFromJSON(publicKey),
    }),
  );
}

/** Signs the API's sign-in challenge with a passkey. */
export async function getPasskeyCredential(options: string) {
  const { publicKey }: Options<PublicKeyCredentialRequestOptionsJSON> = JSON.parse(options);
  return toJson(
    await navigator.credentials.get({
      publicKey: PublicKeyCredential.parseRequestOptionsFromJSON(publicKey),
    }),
  );
}
