
const utils = {}

utils.init = () => {
    utils.preloadCache()
}

utils.stripProxyFromErrors = (handler = {}) => {
    const newHandler = {
        setPrototypeOf: function(target, proto) {
            if (proto === null)
                throw new TypeError('Cannot convert object to primitive value')
            if (Object.getPrototypeOf(target) === Object.getPrototypeOf(proto)) {
                throw new TypeError('Cyclic __proto__ value')
            }
            return Reflect.setPrototypeOf(target, proto)
        }
    }
    // We wrap each trap in the handler in a try/catch and modify the error stack if they throw
    const traps = Object.getOwnPropertyNames(handler)
    traps.forEach(trap => {
        newHandler[trap] = function() {
            try {
                // Forward the call to the defined proxy handler
                return handler[trap].apply(this, arguments || [])
            } catch (err) {
                // Stack traces differ per browser, we only support chromium based ones currently
                if (!err || !err.stack || !err.stack.includes(`at `)) {
                    throw err
                }

                // When something throws within one of our traps the Proxy will show up in error stacks
                // An earlier implementation of this code would simply strip lines with a blacklist,
                // but it makes sense to be more surgical here and only remove lines related to our Proxy.
                // We try to use a known "anchor" line for that and strip it with everything above it.
                // If the anchor line cannot be found for some reason we fall back to our blacklist approach.

                const stripWithBlacklist = (stack, stripFirstLine = true) => {
                    const blacklist = [
                        `at Reflect.${trap} `, // e.g. Reflect.get or Reflect.apply
                        `at Object.${trap} `, // e.g. Object.get or Object.apply
                        `at Object.newHandler.<computed> [as ${trap}] ` // caused by this very wrapper :-)
                    ]
                    return (
                        err.stack
                            .split('\n')
                            // Always remove the first (file) line in the stack (guaranteed to be our proxy)
                            .filter((line, index) => !(index === 1 && stripFirstLine))
                            // Check if the line starts with one of our blacklisted strings
                            .filter(line => !blacklist.some(bl => line.trim().startsWith(bl)))
                            .join('\n')
                    )
                }

                const stripWithAnchor = (stack, anchor) => {
                    const stackArr = stack.split('\n')
                    anchor = anchor || `at Object.newHandler.<computed> [as ${trap}] ` // Known first Proxy line in chromium
                    const anchorIndex = stackArr.findIndex(line =>
                        line.trim().startsWith(anchor)
                    )
                    if (anchorIndex === -1) {
                        return false // 404, anchor not found
                    }
                    // Strip everything from the top until we reach the anchor line
                    // Note: We're keeping the 1st line (zero index) as it's unrelated (e.g. `TypeError`)
                    stackArr.splice(1, anchorIndex)
                    return stackArr.join('\n')
                }

                // Special cases due to our nested toString proxies
                err.stack = err.stack.replace(
                    'at Object.toString (',
                    'at Function.toString ('
                )
                if ((err.stack || '').includes('at Function.toString (')) {
                    err.stack = stripWithBlacklist(err.stack, false)
                    throw err
                }

                // Try using the anchor method, fallback to blacklist if necessary
                err.stack = stripWithAnchor(err.stack) || stripWithBlacklist(err.stack)

                throw err // Re-throw our now sanitized error
            }
        }
    })
    return newHandler
}

utils.stripErrorWithAnchor = (err, anchor) => {
    const stackArr = err.stack.split('\n')
    const anchorIndex = stackArr.findIndex(line => line.trim().startsWith(anchor))
    if (anchorIndex === -1) {
        return err // 404, anchor not found
    }
    // Strip everything from the top until we reach the anchor line (remove anchor line as well)
    // Note: We're keeping the 1st line (zero index) as it's unrelated (e.g. `TypeError`)
    stackArr.splice(1, anchorIndex)
    err.stack = stackArr.join('\n')
    return err
}

utils.replaceProperty = (obj, propName, descriptorOverrides = {}) => {
    return Object.defineProperty(obj, propName, {
        // Copy over the existing descriptors (writable, enumerable, configurable, etc)
        ...(Object.getOwnPropertyDescriptor(obj, propName) || {}),
        // Add our overrides (e.g. value, get())
        ...descriptorOverrides
    })
}

utils.preloadCache = () => {
    if (utils.cache) {
        return
    }
    utils.cache = {
        // Used in our proxies
        Reflect: {
            get: Reflect.get.bind(Reflect),
            apply: Reflect.apply.bind(Reflect)
        },
        // Used in `makeNativeString`
        nativeToStringStr: Function.toString + '' // => `function toString() { [native code] }`
    }
}

utils.makeNativeString = (name = '') => {
    return utils.cache.nativeToStringStr.replace('toString', name || '')
}

utils.patchToString = (obj, str = '') => {
    const handler = {
        apply: function(target, ctx) {
            // This fixes e.g. `HTMLMediaElement.prototype.canPlayType.toString + ""`
            if (ctx === Function.prototype.toString) {
                return utils.makeNativeString('toString')
            }
            // `toString` targeted at our proxied Object detected
            if (ctx === obj) {
                // We either return the optional string verbatim or derive the most desired result automatically
                return str || utils.makeNativeString(obj.name)
            }
            // Check if the toString protype of the context is the same as the global prototype,
            // if not indicates that we are doing a check across different windows., e.g. the iframeWithdirect` test case
            const hasSameProto = Object.getPrototypeOf(
                Function.prototype.toString
            ).isPrototypeOf(ctx.toString) // eslint-disable-line no-prototype-builtins
            if (!hasSameProto) {
                // Pass the call on to the local Function.prototype.toString instead
                return ctx.toString()
            }
            return target.call(ctx)
        }
    }

    const toStringProxy = new Proxy(
        Function.prototype.toString,
        utils.stripProxyFromErrors(handler)
    )
    utils.replaceProperty(Function.prototype, 'toString', {
        value: toStringProxy
    })
}

utils.patchToStringNested = (obj = {}) => {
    return utils.execRecursively(obj, ['function'], utils.patchToString)
}

utils.redirectToString = (proxyObj, originalObj) => {
    const handler = {
        apply: function(target, ctx) {
            // This fixes e.g. `HTMLMediaElement.prototype.canPlayType.toString + ""`
            if (ctx === Function.prototype.toString) {
                return utils.makeNativeString('toString')
            }

            // `toString` targeted at our proxied Object detected
            if (ctx === proxyObj) {
                const fallback = () =>
                    originalObj && originalObj.name
                        ? utils.makeNativeString(originalObj.name)
                        : utils.makeNativeString(proxyObj.name)

                // Return the toString representation of our original object if possible
                return originalObj + '' || fallback()
            }

            if (typeof ctx === 'undefined' || ctx === null) {
                return target.call(ctx)
            }

            // Check if the toString protype of the context is the same as the global prototype,
            // if not indicates that we are doing a check across different windows., e.g. the iframeWithdirect` test case
            const hasSameProto = Object.getPrototypeOf(
                Function.prototype.toString
            ).isPrototypeOf(ctx.toString) // eslint-disable-line no-prototype-builtins
            if (!hasSameProto) {
                // Pass the call on to the local Function.prototype.toString instead
                return ctx.toString()
            }

            return target.call(ctx)
        }
    }

    const toStringProxy = new Proxy(
        Function.prototype.toString,
        utils.stripProxyFromErrors(handler)
    )
    utils.replaceProperty(Function.prototype, 'toString', {
        value: toStringProxy
    })
}

utils.replaceWithProxy = (obj, propName, handler) => {
    const originalObj = obj[propName]
    const proxyObj = new Proxy(obj[propName], utils.stripProxyFromErrors(handler))

    utils.replaceProperty(obj, propName, { value: proxyObj })
    utils.redirectToString(proxyObj, originalObj)

    return true
}
utils.replaceGetterWithProxy = (obj, propName, handler) => {
    const fn = Object.getOwnPropertyDescriptor(obj, propName).get
    const fnStr = fn.toString() // special getter function string
    const proxyObj = new Proxy(fn, utils.stripProxyFromErrors(handler))

    utils.replaceProperty(obj, propName, { get: proxyObj })
    utils.patchToString(proxyObj, fnStr)

    return true
}

utils.replaceGetterSetter = (obj, propName, handlerGetterSetter) => {
    const ownPropertyDescriptor = Object.getOwnPropertyDescriptor(obj, propName)
    const handler = { ...ownPropertyDescriptor }

    if (handlerGetterSetter.get !== undefined) {
        const nativeFn = ownPropertyDescriptor.get
        handler.get = function() {
            return handlerGetterSetter.get.call(this, nativeFn.bind(this))
        }
        utils.redirectToString(handler.get, nativeFn)
    }

    if (handlerGetterSetter.set !== undefined) {
        const nativeFn = ownPropertyDescriptor.set
        handler.set = function(newValue) {
            handlerGetterSetter.set.call(this, newValue, nativeFn.bind(this))
        }
        utils.redirectToString(handler.set, nativeFn)
    }

    Object.defineProperty(obj, propName, handler)
}

utils.mockWithProxy = (obj, propName, pseudoTarget, handler) => {
    const proxyObj = new Proxy(pseudoTarget, utils.stripProxyFromErrors(handler))

    utils.replaceProperty(obj, propName, { value: proxyObj })
    utils.patchToString(proxyObj)

    return true
}

utils.createProxy = (pseudoTarget, handler) => {
    const proxyObj = new Proxy(pseudoTarget, utils.stripProxyFromErrors(handler))
    utils.patchToString(proxyObj)

    return proxyObj
}

utils.splitObjPath = objPath => ({
    // Remove last dot entry (property) ==> `HTMLMediaElement.prototype`
    objName: objPath.split('.').slice(0, -1).join('.'),
    // Extract last dot entry ==> `canPlayType`
    propName: objPath.split('.').slice(-1)[0]
})

utils.replaceObjPathWithProxy = (objPath, handler) => {
    const { objName, propName } = utils.splitObjPath(objPath)
    const obj = eval(objName) // eslint-disable-line no-eval
    return utils.replaceWithProxy(obj, propName, handler)
}

utils.execRecursively = (obj = {}, typeFilter = [], fn) => {
    function recurse(obj) {
        for (const key in obj) {
            if (obj[key] === undefined) {
                continue
            }
            if (obj[key] && typeof obj[key] === 'object') {
                recurse(obj[key])
            } else {
                if (obj[key] && typeFilter.includes(typeof obj[key])) {
                    fn.call(this, obj[key])
                }
            }
        }
    }
    recurse(obj)
    return obj
}

utils.stringifyFns = (fnObj = { hello: () => 'world' }) => {
    // Object.fromEntries() ponyfill (in 6 lines) - supported only in Node v12+, modern browsers are fine
    // https://github.com/feross/fromentries
    function fromEntries(iterable) {
        return [...iterable].reduce((obj, [key, val]) => {
            obj[key] = val
            return obj
        }, {})
    }
    return (Object.fromEntries || fromEntries)(
        Object.entries(fnObj)
            .filter(([key, value]) => typeof value === 'function')
            .map(([key, value]) => [key, value.toString()]) // eslint-disable-line no-eval
    )
}

utils.materializeFns = (fnStrObj = { hello: "() => 'world'" }) => {
    return Object.fromEntries(
        Object.entries(fnStrObj).map(([key, value]) => {
            if (value.startsWith('function')) {
                // some trickery is needed to make oldschool functions work :-)
                return [key, eval(`() => ${value}`)()] // eslint-disable-line no-eval
            } else {
                // arrow functions just work
                return [key, eval(value)] // eslint-disable-line no-eval
            }
        })
    )
}

// Proxy handler templates for re-usability
utils.makeHandler = () => ({
    // Used by simple `navigator` getter evasions
    getterValue: value => ({
        apply(target, ctx, args) {
            // Let's fetch the value first, to trigger and escalate potential errors
            // Illegal invocations like `navigator.__proto__.vendor` will throw here
            utils.cache.Reflect.apply(...arguments)
            return value
        }
    })
})

utils.arrayEquals = (array1, array2) => {
    if (array1.length !== array2.length) {
        return false
    }
    for (let i = 0; i < array1.length; ++i) {
        if (array1[i] !== array2[i]) {
            return false
        }
    }
    return true
}

utils.memoize = fn => {
    const cache = []
    return function(...args) {
        if (!cache.some(c => utils.arrayEquals(c.key, args))) {
            cache.push({ key: args, value: fn.apply(this, args) })
        }
        return cache.find(c => utils.arrayEquals(c.key, args)).value
    }
}

function applyScript(script) {
    try {
        script();
    } catch (err) {
        console.error(err)
        // setInterval(() => {
        //     document.body.innerHTML = err.toString();
        // }, 1);
    }
}

applyScript(() => {
    // chrome.app
    window.chrome = {}
    window.chrome.app = {
        "isInstalled": false,
        "InstallState": {
            "DISABLED": "disabled",
            "INSTALLED": "installed",
            "NOT_INSTALLED": "not_installed"
        },
        "RunningState": {
            "CANNOT_RUN": "cannot_run",
            "READY_TO_RUN": "ready_to_run",
            "RUNNING": "running"
        },
        get isInstalled() {
            return false
        },
        getDetails: function getDetails() {
            if (arguments.length) {
                throw makeError.ErrorInInvocation(`getDetails`)
            }
            return null
        },
        getIsInstalled: function getDetails() {
            if (arguments.length) {
                throw makeError.ErrorInInvocation(`getIsInstalled`)
            }
            return false
        },
        runningState: function getDetails() {
            if (arguments.length) {
                throw makeError.ErrorInInvocation(`runningState`)
            }
            return 'cannot_run'
        }
    }
});

applyScript(() => {
    if (!window.chrome) {
        // Use the exact property descriptor found in headful Chrome
        // fetch it via `Object.getOwnPropertyDescriptor(window, 'chrome')`
        Object.defineProperty(window, 'chrome', {
            writable: true,
            enumerable: true,
            configurable: false, // note!
            value: {} // We'll extend that later
        })
    }

    // That means we're running headful and don't need to mock anything
    if ('csi' in window.chrome) {
        return // Nothing to do here
    }

    // Check that the Navigation Timing API v1 is available, we need that
    if (!window.performance || !window.performance.timing) {
        return
    }

    const { timing } = window.performance

    window.chrome.csi = function() {
        return {
            onloadT: timing.domContentLoadedEventEnd,
            startE: timing.navigationStart,
            pageT: Date.now() - timing.navigationStart,
            tran: 15 // Transition type or something
        }
    }
});

applyScript(() => {
    if (!window.chrome) {
        // Use the exact property descriptor found in headful Chrome
        // fetch it via `Object.getOwnPropertyDescriptor(window, 'chrome')`
        Object.defineProperty(window, 'chrome', {
            writable: true,
            enumerable: true,
            configurable: false, // note!
            value: {} // We'll extend that later
        })
    }

    // That means we're running headful and don't need to mock anything
    if ('loadTimes' in window.chrome) {
        return // Nothing to do here
    }

    // Check that the Navigation Timing API v1 + v2 is available, we need that
    if (
        !window.performance ||
        !window.performance.timing ||
        !window.PerformancePaintTiming
    ) {
        return
    }

    const { performance } = window

    // Some stuff is not available on about:blank as it requires a navigation to occur,
    // let's harden the code to not fail then:
    const ntEntryFallback = {
        nextHopProtocol: 'h2',
        type: 'other'
    }

    // The API exposes some funky info regarding the connection
    const protocolInfo = {
        get connectionInfo() {
            const ntEntry =
                performance.getEntriesByType('navigation')[0] || ntEntryFallback
            return ntEntry.nextHopProtocol
        },
        get npnNegotiatedProtocol() {
            // NPN is deprecated in favor of ALPN, but this implementation returns the
            // HTTP/2 or HTTP2+QUIC/39 requests negotiated via ALPN.
            const ntEntry =
                performance.getEntriesByType('navigation')[0] || ntEntryFallback
            return ['h2', 'hq'].includes(ntEntry.nextHopProtocol)
                ? ntEntry.nextHopProtocol
                : 'unknown'
        },
        get navigationType() {
            const ntEntry =
                performance.getEntriesByType('navigation')[0] || ntEntryFallback
            return ntEntry.type
        },
        get wasAlternateProtocolAvailable() {
            // The Alternate-Protocol header is deprecated in favor of Alt-Svc
            // (https://www.mnot.net/blog/2016/03/09/alt-svc), so technically this
            // should always return false.
            return false
        },
        get wasFetchedViaSpdy() {
            // SPDY is deprecated in favor of HTTP/2, but this implementation returns
            // true for HTTP/2 or HTTP2+QUIC/39 as well.
            const ntEntry =
                performance.getEntriesByType('navigation')[0] || ntEntryFallback
            return ['h2', 'hq'].includes(ntEntry.nextHopProtocol)
        },
        get wasNpnNegotiated() {
            // NPN is deprecated in favor of ALPN, but this implementation returns true
            // for HTTP/2 or HTTP2+QUIC/39 requests negotiated via ALPN.
            const ntEntry =
                performance.getEntriesByType('navigation')[0] || ntEntryFallback
            return ['h2', 'hq'].includes(ntEntry.nextHopProtocol)
        }
    }

    const { timing } = window.performance

    // Truncate number to specific number of decimals, most of the `loadTimes` stuff has 3
    function toFixed(num, fixed) {
        var re = new RegExp('^-?\\d+(?:.\\d{0,' + (fixed || -1) + '})?')
        return num.toString().match(re)[0]
    }

    const timingInfo = {
        get firstPaintAfterLoadTime() {
            // This was never actually implemented and always returns 0.
            return 0
        },
        get requestTime() {
            return timing.navigationStart / 1000
        },
        get startLoadTime() {
            return timing.navigationStart / 1000
        },
        get commitLoadTime() {
            return timing.responseStart / 1000
        },
        get finishDocumentLoadTime() {
            return timing.domContentLoadedEventEnd / 1000
        },
        get finishLoadTime() {
            return timing.loadEventEnd / 1000
        },
        get firstPaintTime() {
            const fpEntry = performance.getEntriesByType('paint')[0] || {
                startTime: timing.loadEventEnd / 1000 // Fallback if no navigation occured (`about:blank`)
            }
            return toFixed(
                (fpEntry.startTime + performance.timeOrigin) / 1000,
                3
            )
        }
    }

    window.chrome.loadTimes = function() {
        return {
            ...protocolInfo,
            ...timingInfo
        }
    }
});

const STATIC_DATA = {
    "OnInstalledReason": {
        "CHROME_UPDATE": "chrome_update",
        "INSTALL": "install",
        "SHARED_MODULE_UPDATE": "shared_module_update",
        "UPDATE": "update"
    },
    "OnRestartRequiredReason": {
        "APP_UPDATE": "app_update",
        "OS_UPDATE": "os_update",
        "PERIODIC": "periodic"
    },
    "PlatformArch": {
        "ARM": "arm",
        "ARM64": "arm64",
        "MIPS": "mips",
        "MIPS64": "mips64",
        "X86_32": "x86-32",
        "X86_64": "x86-64"
    },
    "PlatformNaclArch": {
        "ARM": "arm",
        "MIPS": "mips",
        "MIPS64": "mips64",
        "X86_32": "x86-32",
        "X86_64": "x86-64"
    },
    "PlatformOs": {
        "ANDROID": "android",
        "CROS": "cros",
        "LINUX": "linux",
        "MAC": "mac",
        "OPENBSD": "openbsd",
        "WIN": "win"
    },
    "RequestUpdateCheckStatus": {
        "NO_UPDATE": "no_update",
        "THROTTLED": "throttled",
        "UPDATE_AVAILABLE": "update_available"
    }
};

applyScript(() => {
    window.chrome.runtime = {
        // There's a bunch of static data in that property which doesn't seem to change,
        // we should periodically check for updates: `JSON.stringify(window.chrome.runtime, null, 2)`
        ...STATIC_DATA,
        // `chrome.runtime.id` is extension related and returns undefined in Chrome
        get id() {
            return undefined
        },
        // These two require more sophisticated mocks
        connect: null,
        sendMessage: null
    }
});


applyScript(() => {
    const parseInput = arg => {
        const [mime, codecStr] = arg.trim().split(';')
        let codecs = []
        if (codecStr && codecStr.includes('codecs="')) {
            codecs = codecStr
                .trim()
                .replace(`codecs="`, '')
                .replace(`"`, '')
                .trim()
                .split(',')
                .filter(x => !!x)
                .map(x => x.trim())
        }
        return {
            mime,
            codecStr,
            codecs
        }
    }

    const canPlayType = {
        // Intercept certain requests
        apply: function(target, ctx, args) {
            if (!args || !args.length) {
                return target.apply(ctx, args)
            }
            const { mime, codecs } = parseInput(args[0])
            // This specific mp4 codec is missing in Chromium
            if (mime === 'video/mp4') {
                if (codecs.includes('avc1.42E01E')) {
                    return 'probably'
                }
            }
            // This mimetype is only supported if no codecs are specified
            if (mime === 'audio/x-m4a' && !codecs.length) {
                return 'maybe'
            }

            // This mimetype is only supported if no codecs are specified
            if (mime === 'audio/aac' && !codecs.length) {
                return 'probably'
            }
            // Everything else as usual
            return target.apply(ctx, args)
        }
    }

    /* global HTMLMediaElement */
    utils.replaceWithProxy(
        HTMLMediaElement.prototype,
        'canPlayType',
        canPlayType
    )
});


applyScript(() => {
    const addContentWindowProxy = iframe => {
        const contentWindowProxy = {
            get(target, key) {
                // Now to the interesting part:
                // We actually make this thing behave like a regular iframe window,
                // by intercepting calls to e.g. `.self` and redirect it to the correct thing. :)
                // That makes it possible for these assertions to be correct:
                // iframe.contentWindow.self === window.top // must be false
                if (key === 'self') {
                    return this
                }
                // iframe.contentWindow.frameElement === iframe // must be true
                if (key === 'frameElement') {
                    return iframe
                }
                // Intercept iframe.contentWindow[0] to hide the property 0 added by the proxy.
                if (key === '0') {
                    return undefined
                }
                return Reflect.get(target, key)
            }
        }

        if (!iframe.contentWindow) {
            const proxy = new Proxy(window, contentWindowProxy)
            Object.defineProperty(iframe, 'contentWindow', {
                get() {
                    return proxy
                },
                set(newValue) {
                    return newValue // contentWindow is immutable
                },
                enumerable: true,
                configurable: false
            })
        }
    }

    // Handles iframe element creation, augments `srcdoc` property so we can intercept further
    const handleIframeCreation = (target, thisArg, args) => {
        const iframe = target.apply(thisArg, args)

        // We need to keep the originals around
        const _iframe = iframe
        const _srcdoc = _iframe.srcdoc

        // Add hook for the srcdoc property
        // We need to be very surgical here to not break other iframes by accident
        Object.defineProperty(iframe, 'srcdoc', {
            configurable: true, // Important, so we can reset this later
            get: function() {
                return _srcdoc
            },
            set: function(newValue) {
                addContentWindowProxy(this)
                // Reset property, the hook is only needed once
                Object.defineProperty(iframe, 'srcdoc', {
                    configurable: false,
                    writable: false,
                    value: _srcdoc
                })
                _iframe.srcdoc = newValue
            }
        })
        return iframe
    }

    // Adds a hook to intercept iframe creation events
    const addIframeCreationSniffer = () => {
        /* global document */
        const createElementHandler = {
            // Make toString() native
            get(target, key) {
                return Reflect.get(target, key)
            },
            apply: function(target, thisArg, args) {
                const isIframe =
                    args && args.length && `${args[0]}`.toLowerCase() === 'iframe'
                if (!isIframe) {
                    // Everything as usual
                    return target.apply(thisArg, args)
                } else {
                    return handleIframeCreation(target, thisArg, args)
                }
            }
        }
        // All this just due to iframes with srcdoc bug
        utils.replaceWithProxy(
            document,
            'createElement',
            createElementHandler
        )
    }

    // Let's go
    addIframeCreationSniffer()
});
applyScript(() => {
    /**
     * Input might look funky, we need to normalize it so e.g. whitespace isn't an issue for our spoofing.
     *
     * @example
     * video/webm; codecs="vp8, vorbis"
     * video/mp4; codecs="avc1.42E01E"
     * audio/x-m4a;
     * audio/ogg; codecs="vorbis"
     * @param {String} arg
     */
    const parseInput = arg => {
        const [mime, codecStr] = arg.trim().split(';')
        let codecs = []
        if (codecStr && codecStr.includes('codecs="')) {
            codecs = codecStr
                .trim()
                .replace(`codecs="`, '')
                .replace(`"`, '')
                .trim()
                .split(',')
                .filter(x => !!x)
                .map(x => x.trim())
        }
        return {
            mime,
            codecStr,
            codecs
        }
    }

    const canPlayType = {
        // Intercept certain requests
        apply: function(target, ctx, args) {
            if (!args || !args.length) {
                return target.apply(ctx, args)
            }
            const { mime, codecs } = parseInput(args[0])
            // This specific mp4 codec is missing in Chromium
            if (mime === 'video/mp4') {
                if (codecs.includes('avc1.42E01E')) {
                    return 'probably'
                }
            }
            // This mimetype is only supported if no codecs are specified
            if (mime === 'audio/x-m4a' && !codecs.length) {
                return 'maybe'
            }

            // This mimetype is only supported if no codecs are specified
            if (mime === 'audio/aac' && !codecs.length) {
                return 'probably'
            }
            // Everything else as usual
            return target.apply(ctx, args)
        }
    }

    /* global HTMLMediaElement */
    utils.replaceWithProxy(
        HTMLMediaElement.prototype,
        'canPlayType',
        canPlayType
    )
});

applyScript(() => {
    utils.replaceGetterWithProxy(
        Object.getPrototypeOf(navigator),
        'hardwareConcurrency',
        utils.makeHandler().getterValue(2)
    )
});

applyScript(() => {
    const languages = ['en-US', 'en']
    utils.replaceGetterWithProxy(
        Object.getPrototypeOf(navigator),
        'languages',
        utils.makeHandler().getterValue(Object.freeze([...languages]))
    )
});


applyScript(() => {
    const isSecure = document.location.protocol.startsWith('https')

    // In headful on secure origins the permission should be "default", not "denied"
    if (isSecure) {
        utils.replaceGetterWithProxy(Notification, 'permission', {
            apply() {
                return 'default'
            }
        })
    }

    // Another weird behavior:
    // On insecure origins in headful the state is "denied",
    // whereas in headless it's "prompt"
    if (!isSecure) {
        const handler = {
            apply(target, ctx, args) {
                const param = (args || [])[0]

                const isNotifications =
                    param && param.name && param.name === 'notifications'
                if (!isNotifications) {
                    return utils.cache.Reflect.apply(...arguments)
                }

                return Promise.resolve(
                    Object.setPrototypeOf(
                        {
                            state: 'denied',
                            onchange: null
                        },
                        PermissionStatus.prototype
                    )
                )
            }
        }
        // Note: Don't use `Object.getPrototypeOf` here
        utils.replaceWithProxy(Permissions.prototype, 'query', handler)
    }
});

applyScript(() => {
    utils.replaceGetterWithProxy(
        Object.getPrototypeOf(navigator),
        'vendor',
        utils.makeHandler().getterValue('Google Inc.')
    )
});

applyScript(() => {
    if (navigator.webdriver === false) {
        // Post Chrome 89.0.4339.0 and already good
    } else if (navigator.webdriver === undefined) {
        // Pre Chrome 89.0.4339.0 and already good
    } else {
        // Pre Chrome 88.0.4291.0 and needs patching
        delete Object.getPrototypeOf(navigator).webdriver
    }
});
