export const API_BASE = `${window.location.protocol}//${window.location.hostname}:3000`;

export async function fetchJson(url: string) {
    const response = await fetch(`${API_BASE}${url}`);

    if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
    }

    return await response.json();
}
