import { useEffect, useMemo, useRef, useState } from 'react';
import { useDispatch, useSelector } from 'react-redux';
import { toast } from 'react-hot-toast';
import * as api from '../api';
import * as bmarksSlice from '../store/bmarksSlice';
import { RootState } from '../store';
import { Bmark } from '../api';
import isEqual from 'lodash.isequal';
import store from '../store';
import { SettingsState } from '../settings';

interface UseBookmarkSearchProps {
    settings: SettingsState;
    settingsUpdated: number;
    showAll: boolean;
}

export function useBookmarkSearch({ settings, settingsUpdated, showAll }: UseBookmarkSearchProps) {
    const dispatch = useDispatch();
    const bmarks = useSelector((state: RootState) => state.bmarks.value);

    // Search form state
    const [inputTags, _setInputTags] = useState("");
    const [inputTitle, _setInputTitle] = useState("");
    const [inputUrl, _setInputUrl] = useState("");
    const [inputDescription, _setInputDescription] = useState("");
    const [inputKeyword, _setInputKeyword] = useState("");
    const [inputSemantic, _setInputSemantic] = useState("");
    const [isSearching, setIsSearching] = useState(false);

    const formRefs = useRef({
        inputTags: inputTags,
        inputTitle: inputTitle,
        inputUrl: inputUrl,
        inputDescription: inputDescription
    });

    const setInputTags = (val: string) => {
        formRefs.current.inputTags = val;
        return _setInputTags(val);
    };

    const setInputTitle = (val: string) => {
        formRefs.current.inputTitle = val;
        return _setInputTitle(val);
    };

    const setInputUrl = (val: string) => {
        formRefs.current.inputUrl = val;
        return _setInputUrl(val);
    };

    const setInputDescription = (val: string) => {
        formRefs.current.inputDescription = val;
        return _setInputDescription(val);
    };

    const setInputKeyword = (val: string) => {
        return _setInputKeyword(val);
    };

    const setInputSemantic = (val: string) => {
        return _setInputSemantic(val);
    };

    const getBmarks = async (props: {
        tags: string,
        title: string,
        url: string,
        description: string,
        keyword: string,
        semantic: string,
    }) => {
        const tagsFetch = props.tags.trim().replaceAll(" ", ",").split(",");

        const shouldRefresh = props.tags.length
            || props.title
            || props.url
            || props.description
            || props.keyword
            || props.semantic
            || showAll;

        if (!shouldRefresh) {
            return [];
        }

        return api.fetchBmarks({
            tags: tagsFetch.join(","),
            title: props.title,
            url: props.url,
            description: props.description,
            keyword: props.keyword,
            semantic: props.semantic || undefined,
        }).then(b => b.reverse());
    };

    function updateBmarksIfNeeded(bmarks: Bmark[]) {
        const excluded = bmarks;
        if (!isEqual(store.getState().bmarks.value, excluded)) {
            dispatch(bmarksSlice.updateAll(excluded));
            return true;
        }
        return false;
    }

    const refreshBmarks = () => {
        setIsSearching(true);
        return getBmarks({
            tags: inputTags,
            title: inputTitle,
            description: inputDescription,
            url: inputUrl,
            keyword: inputKeyword,
            semantic: inputSemantic,
        })
            .then(updateBmarksIfNeeded)
            .catch((err: Error) => {
                const msg = err.message;
                // Detect semantic-specific errors from backend error codes
                if (msg.includes('SEMANTIC_DISABLED')) {
                    toast.error('Semantic search is disabled', { duration: 5000 });
                } else if (msg.includes('INVALID_THRESHOLD')) {
                    toast.error('Invalid similarity threshold', { duration: 5000 });
                } else if (msg.includes('MODEL_UNAVAILABLE')) {
                    toast.error('Semantic model unavailable', { duration: 5000 });
                } else {
                    // Re-throw non-semantic errors
                    throw err;
                }
            })
            .finally(() => setIsSearching(false));
    };

    function excludeHiddenTags(bmarks: Bmark[]) {
        const currentWorkspace = settings.workspaceState.workspaces[settings.workspaceState.currentWorkspace];
        const { blacklist, whitelist } = currentWorkspace.tags;

        if (whitelist.length > 0) {
            return bmarks.filter(bmark => bmark.tags.some(t => whitelist.find(wt => t === wt))).filter(bmark => {
                return !bmark.tags.some(t => blacklist.find(wt => t === wt));
            });
        }

        return bmarks.filter(bmark => {
            return !bmark.tags.some(t => blacklist.find(wt => t === wt));
        });
    }

    const bmarksFiltered = useMemo(() => {
        return excludeHiddenTags(bmarks);
    }, [bmarks, settingsUpdated, settings.workspaceState.currentWorkspace]);

    return {
        // State
        inputTags,
        inputTitle,
        inputUrl,
        inputDescription,
        inputKeyword,
        inputSemantic,
        bmarksFiltered,
        isSearching,

        // Setters
        setInputTags,
        setInputTitle,
        setInputUrl,
        setInputDescription,
        setInputKeyword,
        setInputSemantic,

        // Actions
        refreshBmarks,
        getBmarks,
        updateBmarksIfNeeded,
        excludeHiddenTags,
    };
}