import { startsWith } from "lodash";
import { useEffect, useMemo, useRef, useState } from "react";
import AutosizeInput from 'react-input-autosize';


const MAX_TAGS_AUTOCOMPLETE = 7;

export function TagInput(props: {
    onKeyDown?: (e: React.KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>) => void;
    onValue?: (e: string[]) => void;
    defaultValue?: string;

    tagList: string[];
    hiddenTags: string[];
    tagListRequested?: () => void;
    autoFocus?: boolean;

    isSearch?: boolean;
    listenEvent?: boolean;
    autoSize?: boolean;

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

    const className = "transition-all bg-gray-700 hover:bg-gray-600/90 focus:bg-gray-500 shadow-sm hover:shadow-inner focus:shadow-inner text-gray-100 rounded outline-0 p-1 px-2 text-gray-100 max-w-96 w-full " + (props.className ?? "");

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
            setValue(valueNew);
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

    const currTypingWord = (): [undefined, -1] | [string, number] => {
        if (!el.current) return [undefined, -1];

        const tagInputList = value.split(" ");
        const [selectionStart, selectionEnd] = [el.current.selectionStart ?? 0, el.current.selectionEnd ?? 0];
        const currTyping = value.substring(0, selectionEnd);
        const currWordIdx = currTyping.split(" ").length - 1;
        const currWord = tagInputList[currWordIdx];
        if (!currWord) {
            return [undefined, -1]
        }

        return [currWord, currWordIdx];
    }


    const tagsFiltered = useMemo(() => {
        if (!el.current) return []

        const tagInputList = value.split(" ");

        let [currWord, _] = currTypingWord();
        if (!currWord) return []

        const negTag = currWord.startsWith("-");
        let currWordClean = currWord;
        if (negTag) {
            currWordClean = currWord.substring(1);
        }

        const visibleTags: string[] = [];
        const hiddenTags = props.hiddenTags?.map(t => negTag ? "-" + t : t);
        tagInputList.forEach(t => {

            let visibleTag = hiddenTags.find(ht => ht === t || t.includes(ht + "/"))
            if (visibleTag) {
                visibleTags.push(visibleTag);
            }
        })

        return props.tagList.map(tag => {
            if (negTag) {
                return "-" + tag;
            }
            return tag;
        }).filter(t => {
            // remove duplicates
            const [_, ...rest] = tagInputList;
            if (rest.includes(t)) {
                return
            }


            // hide hidden tags but show them if they're referenced in the input
            if (
                hiddenTags.find(ht => t == ht || t.includes(ht + "/"))
                && !visibleTags.find(vt => t == vt || t.includes(vt + "/"))
            ) {
                return false
            }
            if (t.includes("servi")) {
                console.log(t, currWord)
                console.log(t, currWordClean as string)
            }

            return t.includes(currWordClean as string);
        })
    }, [value]);

    const rect = el.current?.getBoundingClientRect?.();

    const listHeight = (rect?.height ?? 0);
    const listWidth = ((rect?.width ?? 0));

    const paste = (tag: string, wordIdx: number) => {
        const tagList = value.split(" ");
        if (wordIdx > (tagList.length - 1)) {
            return
        }

        tagList[wordIdx] = tag.trim() + " ";
        const position = [...tagList.slice(0, wordIdx), tagList[wordIdx]].join(" ").length;

        setValue(tagList.join(" "));

        el.current?.blur();
        // HACK: when width limit is reached the caret goes out of view 
        requestAnimationFrame(() => {
            if (!el.current) {
                return
            }
            el.current.selectionStart = position;
            el.current.selectionEnd = position;
            el.current?.focus();
            setIsDirty(false);
        })


    }

    const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (e.key === "ArrowDown") {
            e.preventDefault()
            setSearchIdx((searchIdx + 1) > Math.min((tagsFiltered?.length ?? (MAX_TAGS_AUTOCOMPLETE - 1)) - 1, (MAX_TAGS_AUTOCOMPLETE - 1)) ? 0 : searchIdx + 1)
        }

        if (e.key === "ArrowUp") {
            e.preventDefault()
            setSearchIdx((searchIdx - 1) < 0 ? Math.min((tagsFiltered?.length ?? (MAX_TAGS_AUTOCOMPLETE - 1)) - 1, (MAX_TAGS_AUTOCOMPLETE - 1)) : searchIdx - 1)
        }

        if (e.key === "Tab" || e.key === "Enter") {
            const [currWord, currWordIdx] = currTypingWord();

            if (currWord) {
                const foundTag = tagsFiltered?.[searchIdx];
                if (foundTag) {
                    paste(foundTag, currWordIdx)
                    e.preventDefault();
                    e.stopPropagation();
                    return
                } else if (props.isSearch) {
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

    return <div className="relative !flex">
        {isDirty && focus && tagsFiltered && <div
            style={{ flexDirection: "column", top: listHeight, width: listWidth }}
            className="absolute rounded-md z-50 flex bg-gray-600 w-40 overflow-hidden"
        >
            {tagsFiltered.slice(0, MAX_TAGS_AUTOCOMPLETE).map((tag, idx) => {
                const focused = searchIdx === idx;
                return <div onMouseDown={() => {
                    const [currWord, currWordIdx] = currTypingWord();
                    if (currWord) {
                        paste(tag, currWordIdx)
                    }
                }} className={"bg-gray-800 text-gray-200 px-2 py-1 " + (focused ? "!bg-gray-600" : "")} key={tag}>{tag}</div>
            })}
        </div>}

        {props.autoSize && <AutosizeInput
            onKeyDown={onKeyDown}
            onFocus={onFocus}
            onBlur={onBlur}
            onInput={onInput}
            inputRef={ref => el.current = ref}
            type="text"
            autoComplete="off"
            autoCorrect="off"
            value={value.replace(/[,]{2,}/g, " ").replace(/[/]{2,}/g, "/").replace(/[^\p{L}\p{N} /-]+/ug, "")}
            placeholderIsMinWidth
            extraWidth={15}
            placeholder="Tags"
            className="!flex"
            inputClassName={className + " auto-size"}
            autoFocus={props.autoFocus ?? false}
        />}
        {!props.autoSize && <input
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
        />}

    </div>
}

