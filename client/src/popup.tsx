import { useEffect, useState } from "react";
import { createPortal } from "react-dom";

export default function IframePopup({ url, isOpen, onClose }: { url: string; isOpen: boolean; onClose: () => void }) {
    const [iframeHtml, setIframeHtml] = useState<string>("");
    const [scripts, setScripts] = useState<any[]>([]);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string>("");

    // Process iframe HTML to override width/height attributes
    const processIframeHtml = (html: string): string => {
        // Create a temporary div to parse the HTML
        const tempDiv = document.createElement('div');
        tempDiv.innerHTML = html;

        // Find all iframe elements and modify their attributes
        const iframes = tempDiv.querySelectorAll('iframe');
        iframes.forEach(iframe => {
            // Remove all size constraints
            iframe.removeAttribute('width');
            iframe.removeAttribute('height');
            iframe.removeAttribute('style');

            // Set aggressive sizing to fill container
            iframe.style.cssText = `
                width: 100% !important;
                height: 100% !important;
                min-width: 100% !important;
                min-height: 100% !important;
                max-width: none !important;
                max-height: none !important;
                border: none !important;
                margin: 0 !important;
                padding: 0 !important;
            `;
        });

        return tempDiv.innerHTML;
    };

    // Clean up old cache entries
    const cleanupOldCache = () => {
        const keys = Object.keys(localStorage);
        const microlinkKeys = keys.filter(key => key.startsWith('microlink_'));
        const maxAge = 24 * 60 * 60 * 1000; // 24 hours

        microlinkKeys.forEach(key => {
            try {
                const data = JSON.parse(localStorage.getItem(key) || '{}');
                if (data.timestamp && (Date.now() - data.timestamp) > maxAge) {
                    localStorage.removeItem(key);
                }
            } catch (err) {
                // Remove invalid cache entries
                localStorage.removeItem(key);
            }
        });
    };

    useEffect(() => {
        if (isOpen && url) {
            fetchIframeContent();
        }
    }, [isOpen, url]);

    // Clean up old cache entries when component mounts
    useEffect(() => {
        cleanupOldCache();
    }, []);

    // Handle escape key to close popup
    useEffect(() => {
        const handleEscape = (e: KeyboardEvent) => {
            if (e.key === 'Escape' && isOpen) {
                onClose();
            }
        };

        if (isOpen) {
            document.addEventListener('keydown', handleEscape);
        }

        return () => {
            document.removeEventListener('keydown', handleEscape);
        };
    }, [isOpen, onClose]);

    const fetchIframeContent = async () => {
        setLoading(true);
        setError("");

        // Check localStorage cache first
        const cacheKey = `microlink_${btoa(url)}`;
        const cached = localStorage.getItem(cacheKey);

        if (cached) {
            try {
                const data = JSON.parse(cached);
                // Check if cache is still valid (24 hours)
                const cacheAge = Date.now() - (data.timestamp || 0);
                const maxAge = 24 * 60 * 60 * 1000; // 24 hours

                if (cacheAge < maxAge && data.status === "success" && data.data?.iframe?.html) {
                    // Process cached iframe HTML as well
                    const processedHtml = processIframeHtml(data.data.iframe.html);
                    setIframeHtml(processedHtml);
                    setScripts(data.data.iframe.scripts || []);
                    setLoading(false);
                    return;
                } else if (cacheAge >= maxAge) {
                    // Remove expired cache
                    localStorage.removeItem(cacheKey);
                }
            } catch (err) {
                console.warn("Failed to parse cached data:", err);
                localStorage.removeItem(cacheKey);
            }
        }

        try {
            const microlinkUrl = `https://api.microlink.io/?url=${encodeURIComponent(url)}&iframe=true&meta=false`;
            const response = await fetch(microlinkUrl);
            const data = await response.json();

            if (data.status === "success" && data.data?.iframe?.html) {
                // Cache the successful response with timestamp
                const cacheData = { ...data, timestamp: Date.now() };
                localStorage.setItem(cacheKey, JSON.stringify(cacheData));

                // Process iframe HTML to override width/height attributes
                const processedHtml = processIframeHtml(data.data.iframe.html);
                setIframeHtml(processedHtml);
                setScripts(data.data.iframe.scripts || []);
            } else {
                setError("Failed to load iframe content");
            }
        } catch (err) {
            setError("Error fetching iframe content");
            console.error("Error fetching iframe:", err);
        } finally {
            setLoading(false);
        }
    };

    const injectScripts = () => {
        const injectedScripts: HTMLScriptElement[] = [];
        scripts.forEach(script => {
            // Check if script already exists
            const existingScript = document.querySelector(`script[src="${script.src}"]`);
            if (!existingScript) {
                const scriptElement = document.createElement('script');
                if (script.src) scriptElement.src = script.src;
                if (script.async) scriptElement.async = script.async;
                if (script.charset) scriptElement.charset = script.charset;
                document.head.appendChild(scriptElement);
                injectedScripts.push(scriptElement);
            }
        });
        return injectedScripts;
    };

    useEffect(() => {
        let injectedScripts: HTMLScriptElement[] = [];
        if (scripts.length > 0) {
            injectedScripts = injectScripts();
        }

        // Cleanup function to remove injected scripts when popup closes
        return () => {
            injectedScripts.forEach(script => {
                if (script.parentNode) {
                    script.parentNode.removeChild(script);
                }
            });
        };
    }, [scripts]);

    if (!isOpen) return null;

    // Use Portal to render at document body level
    return createPortal(
        <>
            <style>
                {`
                    @keyframes popupEnter {
                        from {
                            opacity: 0;
                            transform: scale(0.95) translateY(10px);
                        }
                        to {
                            opacity: 1;
                            transform: scale(1) translateY(0);
                        }
                    }
                    
                    .iframe-popup-overlay {
                        position: fixed !important;
                        top: 0 !important;
                        left: 0 !important;
                        right: 0 !important;
                        bottom: 0 !important;
                        z-index: 999999 !important;
                        background: rgba(0, 0, 0, 0.3) !important;
                        backdrop-filter: blur(4px) !important;
                        display: flex !important;
                        align-items: center !important;
                        justify-content: center !important;
                        pointer-events: auto !important;
                    }
                    
                    .iframe-popup-container {
                        z-index: 999999 !important;
                        position: relative !important;
                        pointer-events: auto !important;
                    }
                    
                    /* Force the popup to be on top of everything */
                    body > .iframe-popup-overlay {
                        z-index: 2147483647 !important;
                    }
                    
                    /* Maximum possible z-index */
                    .iframe-popup-overlay {
                        z-index: 2147483647 !important;
                    }
                    
                    /* Ensure pointer events work */
                    .iframe-popup-overlay * {
                        pointer-events: auto !important;
                    }
                `}
            </style>
            <div
                className="iframe-popup-overlay"
                onClick={onClose}
            >
                <div
                    className="bg-gray-900/95 backdrop-blur-md rounded-2xl p-4 h-[95vh] w-full mx-4 border border-gray-700/50 shadow-2xl flex flex-col iframe-popup-container"
                    style={{
                        animation: 'popupEnter 0.3s ease-out'
                    }}
                    onClick={(e) => e.stopPropagation()}
                >
                    <button
                        onClick={onClose}
                        className="absolute top-6 right-6 text-gray-400 hover:text-white text-3xl font-bold transition-colors duration-200 hover:scale-110"
                    >
                        ×
                    </button>

                    {loading && (
                        <div className="flex items-center justify-center flex-1">
                            <div className="text-white flex flex-col items-center gap-4">
                                <div className="animate-spin rounded-full h-12 w-12 border-4 border-gray-600 border-t-white"></div>
                                <div className="text-lg font-medium">Loading iframe content...</div>
                            </div>
                        </div>
                    )}

                    {error && (
                        <div className="text-red-400 text-center flex-1 flex items-center justify-center">
                            <div className="flex flex-col items-center gap-3">
                                <div className="text-6xl">😕</div>
                                <div className="text-xl font-medium">{error}</div>
                            </div>
                        </div>
                    )}

                    {!loading && !error && iframeHtml && (
                        <div
                            className="w-full flex-1 bg-white rounded-xl overflow-hidden shadow-inner"
                            dangerouslySetInnerHTML={{ __html: iframeHtml }}
                        />
                    )}

                    {!loading && !error && !iframeHtml && (
                        <div className="text-gray-400 text-center flex-1 flex items-center justify-center">
                            <div className="flex flex-col items-center gap-3">
                                <div className="text-6xl">📄</div>
                                <div className="text-xl font-medium">No iframe content available for this URL</div>
                            </div>
                        </div>
                    )}
                </div>
            </div>
        </>,
        document.body
    );
}


