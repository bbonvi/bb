import { createSlice, PayloadAction } from '@reduxjs/toolkit';

interface GlobalState {
    editing: number
    focusedIdx: number
    sizes: { [keyof: string]: number };
}

const initialState: GlobalState = {
    editing: -1,
    focusedIdx: -1,
    sizes: {},
};

const GlobalSlice = createSlice({
    name: 'Global',
    initialState,
    reducers: {
        setEditing: (state, action: PayloadAction<number>) => {
            state.editing = action.payload;
        },
        setFocusedIdx: (state, action: PayloadAction<number>) => {
            state.focusedIdx = action.payload;
        },
        setSize: (state, { payload }: PayloadAction<{ id: number, height: number }>) => {
            state.sizes[payload.id.toString()] = payload.height;
        },
        setSizes: (state, action: PayloadAction<{ [keyof: string]: number }>) => {
            state.sizes = action.payload;
        },
    },
});

export const { setEditing, setFocusedIdx, setSize, setSizes } = GlobalSlice.actions;
export default GlobalSlice.reducer;

