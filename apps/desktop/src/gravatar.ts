const gravatarUrls = new Map<string, Promise<string>>();

export type GravatarAuthor = {
  name: string;
  email: string;
};

export function coAuthorsFromCommitBody(body: string): GravatarAuthor[] {
  const authors: GravatarAuthor[] = [];
  const emails = new Set<string>();
  const trailer = /^co-authored-by:\s*(.+?)\s*<([^<>\s]+)>\s*$/gim;

  for (const match of body.matchAll(trailer)) {
    const name = match[1].trim();
    const email = match[2].trim();
    const normalizedEmail = email.toLowerCase();
    if (!name || emails.has(normalizedEmail)) continue;
    emails.add(normalizedEmail);
    authors.push({ name, email });
  }

  return authors;
}

export function gravatarUrl(email: string): Promise<string> {
  const normalizedEmail = email.trim().toLowerCase();
  const cached = gravatarUrls.get(normalizedEmail);
  if (cached) return cached;

  const url = crypto.subtle
    .digest("SHA-256", new TextEncoder().encode(normalizedEmail))
    .then((digest) => {
      const hash = Array.from(new Uint8Array(digest), (byte) =>
        byte.toString(16).padStart(2, "0"),
      ).join("");
      return `https://www.gravatar.com/avatar/${hash}?s=40&d=identicon`;
    });
  gravatarUrls.set(normalizedEmail, url);
  return url;
}
