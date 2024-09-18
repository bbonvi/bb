import { useEffect, useMemo, useRef, useState } from "react";

export function TagInput(props: {
    onKeyDown?: (e: React.KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>) => void;
    onValue?: (e: string[]) => void;
    defaultValue?: string;

    tagList?: string[];
    hiddenTags: string[];
    tagListRequested?: () => void;
    autoFocus?: boolean;

    isSearch?: boolean;
    listenEvent?: boolean;

    className?: string;
}) {
    const [value, setValue] = useState<string>(props.defaultValue ?? "");
    const [focus, setFocus] = useState(false);
    const [isDirty, setIsDirty] = useState(false);
    const [searchIdx, setSearchIdx] = useState(0);

    const el = useRef<HTMLInputElement | null>();

    const {
        listenEvent = false,
        isSearch = false,
    } = props;

    const className = "transition-all bg-gray-700 hover:bg-gray-600/90 focus:bg-gray-500 shadow-sm hover:shadow-inner focus:shadow-inner text-gray-100 rounded outline-0 p-1 px-2 text-gray-100 w-full " + (props.className ?? "");

    useEffect(() => {
        props.onValue?.(value.split(" "))
        setIsDirty(true);
    }, [value]);

    useEffect(() => {
        props.tagListRequested?.();
        requestAnimationFrame(() => { setIsDirty(false) })
    }, []);

    useEffect(() => {
        if (!listenEvent) {
            return
        }

        const onAddSearchTag = (e: Event) => {
            setTimeout(() => el.current?.focus());
            const tag = (e as any).detail;
            const values = value.split(" ");
            const idx = values.indexOf(tag);
            if (idx >= 0) {
                values.splice(idx, 1);
                setValue(values.join(" "))
                return
            }

            let valueNew = (value + " " + tag);
            setValue(valueNew)
            setSearchIdx(0);
        };

        document.addEventListener(
            "add-search-tag",
            onAddSearchTag,
            false,
        );

        return () => {
            document.removeEventListener(
                "add-search-tag",
                onAddSearchTag,
            );
        }
    }, [value, searchIdx, focus]);

    const onInput = (e: React.FormEvent<HTMLInputElement>) => {
        const v = e.currentTarget.value.replace(/[,]{2,}/g, " ").replaceAll(",", " ").replace(/[/]{2,}/g, "/").replace(/[^\p{L}\p{N} /-]+/ug, "");
        setValue(v);
        setSearchIdx(0);
    }

    const tagListSearch = useMemo(() => {
        const tagList = props.tagList ?? [];

        const values = value.split(" ");

        const visibleTags: string[] = [];
        values.forEach(t => {
            let visibleTag = props.hiddenTags?.find(ht => ht === t || t.includes(ht + "/"))
            if (visibleTag) {
                visibleTags.push(visibleTag);
            }
        })

        values.reverse();

        const last = values[0] ?? "";
        if (!last) {
            return
        }

        return tagList.filter(t => {
            // remove duplicates
            const [_, ...rest] = values;
            if (rest.includes(t)) {
                return
            }

            // hide hidden tags but show them if they're referenced in the input
            if (
                props.hiddenTags.find(ht => t == ht || t.includes(ht + "/"))
                && !visibleTags.find(vt => t == vt || t.includes(vt + "/"))
            ) {
                return false
            }

            return t.includes(last);
        })
    }, [value]);

    const rect = el.current?.getBoundingClientRect?.();

    const listTop = ((rect?.top ?? 0) + (rect?.height ?? 0));
    const listLeft = ((rect?.left ?? 0));
    const listWidth = ((rect?.width ?? 0));

    const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (e.key === "ArrowDown") {
            e.preventDefault()
            setSearchIdx((searchIdx + 1) > Math.min((tagListSearch?.length ?? 4) - 1, 4) ? 0 : searchIdx + 1)
        }

        if (e.key === "ArrowUp") {
            e.preventDefault()
            setSearchIdx((searchIdx - 1) < 0 ? Math.min((tagListSearch?.length ?? 4) - 1, 4) : searchIdx - 1)
        }

        // TODO: do we need this?
        // if (e.key === "Enter") {
        //     el.current?.blur();
        //     e.preventDefault();
        // }

        if (e.key === "Tab" || e.key === "Enter") {
            let [first, ...last] = value.split(" ").reverse();
            if (first && !last.length) {
                last = [first]
            }

            if (last.length) {
                let [_, ...rest] = value.split(" ").reverse();
                const found = tagListSearch?.[searchIdx];
                if (found) {
                    setValue([tagListSearch?.[searchIdx], ...rest].reverse().join(" ") + " ")
                    e.preventDefault()
                    return
                }
            }
        }

        props.onKeyDown?.(e);
    }

    const onFocus = () => {
        props.tagListRequested?.();
        setFocus(true);
    }

    const onBlur = () => {
        props.tagListRequested?.();
        setFocus(false);
    }

    return <div>
        {isDirty && focus && tagListSearch && <div
            style={{ flexDirection: "column", top: listTop, left: listLeft, width: listWidth }}
            className="fixed rounded-md z-50 flex bg-gray-600 w-40 overflow-hidden"
        >
            {tagListSearch.slice(0, 5).map((tag, idx) => {
                const focused = searchIdx === idx;
                return <div className={"bg-gray-800 text-gray-200 px-2 py-1 " + (focused ? "!bg-gray-600" : "")} key={tag}>{tag}</div>
            })}
        </div>}
        <input
            onKeyDown={onKeyDown}
            onFocus={onFocus}
            onBlur={onBlur}
            onInput={onInput}
            ref={ref => el.current = ref}
            type="text"
            autoComplete="off"
            autoCorrect="off"
            value={value.replace(/[,]{2,}/g, " ").replace(/[/]{2,}/g, "/").replace(/[^\p{L}\p{N} /-]+/ug, "")}
            placeholder="Tags"
            className={className}
            autoFocus={props.autoFocus ?? false}
        />
    </div>
}

