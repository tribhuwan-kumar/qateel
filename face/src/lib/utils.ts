import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

export type WithoutChild<T> = T extends { child?: any } ? Omit<T, "child"> : T;
export type WithoutChildren<T> = T extends { children?: any } ? Omit<T, "children"> : T;
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>;
export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & { ref?: U | null };

export function truncateText(str: string | undefined, maxLength: number) {
	if (str) {
		if (str.length > maxLength) {
			return str?.slice(0, maxLength) + "...";
		}
	}
  return str;
}

export const truncateMiddle = (str: string, n: number) => {
  if (str.length <= n) return str;
  const half = n/2
  const start = str.slice(0, half);
  const end = str.slice(-half);

  return `${start}...${end}`;
};

function getCookie(name: string): string | undefined {
  if (typeof document === 'undefined') return undefined;
  const value = `; ${document.cookie}`;
  const parts = value.split(`; ${name}=`);
  if (parts.length === 2) return parts.pop()?.split(';').shift();
}

export async function secureFetch(url: string, options: RequestInit = {}) {
  const csrfToken = getCookie('csrf_token');
  const headers = new Headers(options.headers);
  const method = options.method?.toUpperCase() || 'GET';

  if (method !== 'GET' && method !== 'HEAD' && csrfToken) {
    headers.set('X-CSRF-Token', csrfToken);
  }
  if (options.body && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }

  const secureOptions: RequestInit = {
    ...options,
    headers,
    credentials: options.credentials || 'include',
  };

  try {
    const response = await fetch(url, secureOptions);
    return response;
  } catch (error) {
    console.error('Network or Security Error:', error);
    throw error;
  }
}

export function formatSpeed(bytes: string | undefined) {
  if (!bytes || bytes === "0") return '';
  return formatBytes(bytes) + '/s';
}

export function formatBytes(bytes: string | null | undefined) {
	if (!bytes || bytes === "0") return '-';
	const b = parseInt(bytes);
	if (isNaN(b) || b === 0) return '-';
	const k = 1024;
	const sizes = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
	const i = Math.floor(Math.log(b) / Math.log(k));
	return parseFloat((b / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

export function isValidPassword(password: string) {
	return password.length >= 8 && /[a-zA-Z]/.test(password);
}

export function isValidUsername(username: string) {
	return username.length >= 6 && /^[a-zA-Z0-9_]+$/.test(username);
}

