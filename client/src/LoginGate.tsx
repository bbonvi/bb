import { FormEvent, useState } from 'react';

export const AUTH_TOKEN_KEY = 'bb_auth_token';

export function getAuthToken(): string | null {
    return localStorage.getItem(AUTH_TOKEN_KEY);
}

export function setAuthToken(token: string): void {
    localStorage.setItem(AUTH_TOKEN_KEY, token);
}

export function clearAuthToken(): void {
    localStorage.removeItem(AUTH_TOKEN_KEY);
}

interface LoginGateProps {
    onLogin: () => void;
}

export default function LoginGate({ onLogin }: LoginGateProps) {
    const [token, setToken] = useState('');
    const [error, setError] = useState('');

    const handleSubmit = (e: FormEvent) => {
        e.preventDefault();
        const trimmed = token.trim();
        if (!trimmed) {
            setError('Token is required');
            return;
        }
        setAuthToken(trimmed);
        onLogin();
    };

    return (
        <div className="dark:bg-gray-900 dark:text-gray-100 h-screen flex items-center justify-center">
            <div className="bg-gray-800 rounded-lg shadow-lg p-8 w-full max-w-md">
                <h1 className="text-2xl font-semibold mb-6 text-center">Authentication Required</h1>
                <form onSubmit={handleSubmit}>
                    <div className="mb-4">
                        <label htmlFor="auth-token" className="block text-sm text-gray-400 mb-2">
                            API Token
                        </label>
                        <input
                            id="auth-token"
                            type="password"
                            value={token}
                            onChange={(e) => {
                                setToken(e.target.value);
                                setError('');
                            }}
                            placeholder="Enter your token"
                            autoFocus
                            className="w-full transition-all bg-gray-700 hover:bg-gray-600/90 focus:bg-gray-600 text-gray-100 rounded outline-0 p-3 px-4"
                        />
                    </div>
                    {error && (
                        <p className="text-red-400 text-sm mb-4">{error}</p>
                    )}
                    <button
                        type="submit"
                        className="w-full transition-all bg-gray-600 hover:bg-gray-500 shadow-sm hover:shadow-inner rounded p-3 font-medium"
                    >
                        Login
                    </button>
                </form>
            </div>
        </div>
    );
}
