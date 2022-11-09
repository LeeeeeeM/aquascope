import { basicSetup } from "./setup"
import { vim } from "@replit/codemirror-vim"
import {
    EditorView, WidgetType,
    Decoration, DecorationSet,
    ViewUpdate, ViewPlugin
} from "@codemirror/view"
import {
    EditorState, StateField,
    StateEffect, StateEffectType,
    RangeSet, Compartment
} from "@codemirror/state"
import { rust } from "@codemirror/lang-rust"
import { indentUnit } from "@codemirror/language"
import * as ty from "./types"

const initial_code: string =
    `// Please start typing :)

#[derive(Debug, Default)]
struct Box {
    value: i32,
}

impl Box {
    fn inc(&mut self) {
        self.value += 1;
    }

    fn destroy(mut self) {}
}

fn main() {

    let v1 = vec![1, 2, 3];
    v1.push(0);

    let v2 = &mut vec![1, 2, 3];
    v2.push(0);

    let b1 = &Box::default();
    b1.inc();

    let mut b2 = Box::default();
    b2.inc();

    Box::default().destroy();

    println!("Gruëzi, Weltli");
}
`;

let readOnly = new Compartment;
let mainKeybinding = new Compartment;

export interface Icon {
    display: boolean,
    toDom(): HTMLElement
};

interface IconField<C, Ico extends Icon, T> {
    effect_type: StateEffectType<Array<T>>;
    state_field: StateField<DecorationSet>;
    make_decoration(icos: Array<Ico>): Decoration;
    from_output(call_types: C): T;
    sort_field(b1: T, b2: T): number;
};

export class Editor {
    private view: EditorView;

    public constructor(dom: HTMLElement, supported_fields: Array<StateField<DecorationSet>>) {
        let initial_state = EditorState.create({
            doc: initial_code,
            extensions: [
                mainKeybinding.of(vim()),
                basicSetup,
                rust(),
                readOnly.of(EditorState.readOnly.of(false)),
                indentUnit.of("    "),
                ...supported_fields
            ],
        });

        let initial_view = new EditorView({
            state: initial_state,
            parent: dom,
        });

        this.view = initial_view;
    }

    public get_current_contents(): string {
        return this.view.state.doc.toString();
    }

    public toggle_readonly(b: boolean): void {
        this.view.dispatch({
            effects: [readOnly.reconfigure(EditorState.readOnly.of(b))]
        });
    }

    public toggle_vim(b: boolean): void {
        let t = b ? [vim(), basicSetup] : [basicSetup];
        this.view.dispatch({
            effects: [mainKeybinding.reconfigure(t)]
        });
    }

    public remove_icon_field<B, T, Ico extends Icon, F extends IconField<B, Ico, T>>(f: F) {
        this.view.dispatch({
            effects: [ f.effect_type.of([]) ]
        });
    }

    public add_call_types_field<B, T, Ico extends Icon, F extends IconField<B, Ico, T>>(
        f: F,
        method_call_points: Array<B>,
    ) {
        let new_effects = method_call_points.map(f.from_output).sort(f.sort_field);
        console.log(new_effects);
        this.view.dispatch({
            effects: [f.effect_type.of(new_effects)]
        });
    }
}

// ----------------------------------------
// Types to use in an Icon Field

type RGBA = `rgba(${number},${number},${number},${number})`;
type RGB = `rgb(${number},${number},${number})`;

type Color = RGB | RGBA;


// HACK the ending character of the actual type
// might not actually be right before the dereference
// operator `.`, do some testing and then we can probably
// use the character preceding the expected `char_start`.
let range_to_loc = (range: ty.Range): number =>
    range.char_start - 1;

let permission_state_ico_type =
    StateEffect.define<Array<PermissionPoint<TextIco>>>();

type PermissionPoint<I extends Icon> = [I, I, I, number];

let glyph_width = 12;

class TextIco implements Icon {
    constructor(
        readonly display: boolean,
        readonly contents: string,
        readonly expected: boolean,
        readonly actual: boolean
    ) {}

    toDom(): HTMLElement {
        let tt = document.createElementNS("http://www.w3.org/2000/svg", "text");
        tt.classList.add("permission");
        tt.setAttribute("font-family", "IBM Plex Sans");
        tt.setAttribute("font-size", `${glyph_width}px`);
        tt.setAttribute("font-weight", "regular");
        tt.setAttribute("stroke-width", "2");
        tt.textContent = this.contents;
        if (!this.actual) {
            tt.setAttribute("fill-opacity", "0.1");
        }
        return tt as HTMLElement & SVGTextElement;
    }
}

class RWDPermissions<I extends Icon> extends WidgetType {
    constructor(
        readonly read: I,
        readonly write: I,
        readonly drop: I
    ) { super() }

    eq(other: RWDPermissions<I>) {
        return other.read == this.read &&
            other.write == this.write &&
            other.drop == this.drop;
    }

    toDOM() {
        let read_color: RGB = `rgb(${93},${202},${54})`;
        let write_color: RGB = `rgb(${78},${190},${239})`;
        let drop_color: RGB = `rgb(${255},${66},${68})`;
        let all: Array<[I, Color]> =
            [ [this.read, read_color],
              [this.write, write_color],
              [this.drop, drop_color] ];
        let icons: Array<[I, Color]> = all.filter((t) => t[0].display)

        let wrap = document.createElementNS("http://www.w3.org/2000/svg", "svg");
        wrap.classList.add("svg-perm");
        let my_height = icons.length * glyph_width;
        let my_width = 2 * glyph_width;
        wrap.setAttribute("width", `${my_width + 10}px`);
        wrap.setAttribute("height", `${my_height}px`);
        icons.forEach((tup, idx) => {
            let act_ico = tup[0].toDom();
            let exp_ico = tup[0].toDom();

            let y = (idx / icons.length * 100) + (100 / icons.length);

            act_ico.setAttribute("text-anchor", "middle");
            exp_ico.setAttribute("text-anchor", "middle");

            exp_ico.setAttribute("x", "70%")
            exp_ico.setAttribute("y", `${y}%`)
            exp_ico.setAttribute("fill", tup[1])
            exp_ico.setAttribute("stroke", tup[1])

            act_ico.setAttribute("x", "25%")
            act_ico.setAttribute("y", `${y}%`)
            act_ico.setAttribute("fill", tup[1])
            act_ico.setAttribute("stroke", tup[1])

            wrap.appendChild(exp_ico);
            wrap.appendChild(act_ico);
        });
        return wrap as HTMLElement & SVGSVGElement;
    }

    ignoreEvent() { return false }
}

let call_types_to_permissions =
    (perm_info: ty.PermissionsInfo): PermissionPoint<TextIco> => {
        const read_ico = new TextIco(
            perm_info.expected.read,
            "R",
            perm_info.expected.read,
            perm_info.actual.read,
        );
        const write_ico = new TextIco(
            perm_info.expected.write,
            "W",
            perm_info.expected.write,
            perm_info.actual.write,
        );
        const drop_ico = new TextIco(
            perm_info.expected.drop,
            "D",
            perm_info.expected.drop,
            perm_info.actual.drop,
        );
        let loc = range_to_loc(perm_info.range);
        return [read_ico, write_ico, drop_ico, loc];
    };

let make_text_state_field_with_icon = <I extends Icon>(
    ty: StateEffectType<Array<PermissionPoint<I>>>,
    cons: (icos: Array<I>) => Decoration
) => {
    return StateField.define<DecorationSet>({
        create: () => Decoration.none,
        update(points, transactions) {
            console.log(transactions);
            for (let e of transactions.effects) {
                if (e.is(ty)) {
                    return RangeSet.of(e.value.map(([ico_l, ico_m, ico_r, from]) =>
                        cons([ico_l, ico_m, ico_r]).range(from)));
                }
            }

            return points;
        },
        provide: f => EditorView.decorations.from(f),
    })
};

let make_decoration_with_text_ico = <I extends Icon>(icos: Array<I>): Decoration => {
    let fst = icos[0];
    let snd = icos[1];
    let trd = icos[2];
    return Decoration.widget({
        widget: new RWDPermissions<I>(fst, snd, trd),
        side: 0,
    });
};

export let rwd_permissions_field: IconField<ty.PermissionsInfo, TextIco, PermissionPoint<TextIco>> = {
    effect_type: permission_state_ico_type,
    state_field: make_text_state_field_with_icon(
        permission_state_ico_type,
        make_decoration_with_text_ico<TextIco>
    ),
    make_decoration: make_decoration_with_text_ico<TextIco>,
    from_output: call_types_to_permissions,
    sort_field: (b1: PermissionPoint<TextIco>, b2: PermissionPoint<TextIco>): number => {
        if (b1[3] < b2[3]) {
            return -1;
        } else if (b1[3] > b2[3]) {
            return 1;
        } else {
            return 0;
        }
    },
};

// ---------------------------------------
// TODO REMOVE THIS CRAP

// type StackedPointIco<Ico extends Icon> = [Ico, Ico, number];

// type IconToDecoration<I extends Icon> = (l: I, r: I) => Decoration;

// let call_type_to_loc = (call_type: ty.CallTypes): number =>
//     call_type.expected.range.char_start - 1;

// // -------------------------------
// // Icons that differ only in color

// class ColorDiffIco implements Icon {
//     constructor(
//         readonly class_name: string,
//         readonly color_expected: Color,
//         readonly color_actual: Color
//     ) {}

//     toDom(): HTMLElement {
//         let make_name_at_size = (ico_name: string) =>
//             (n: number) => `fa ${ico_name} fa-stack-${n}x`;
//         let wrap = document.createElement("span");
//         wrap.className = "fa-stack small";
//         let box_o = wrap.appendChild(document.createElement("i"));
//         let box_i = wrap.appendChild(document.createElement("i"));
//         let make_style = make_name_at_size(this.class_name);
//         box_o.className = make_style(2);
//         box_i.className = make_style(1);
//         box_o.setAttribute("style", `color: ${this.color_expected};`)
//         box_i.setAttribute("style", `color: ${this.color_actual};`)
//         return wrap;
//     }
// };

// let stacked_color_ico_type = StateEffect.define<Array<StackedPointIco<ColorDiffIco>>>();

// let make_decoration_with_icon = <I extends Icon>(icos: Array<I>): Decoration => {
//     const fst = icos[0];
//     const snd = icos[1];
//     return Decoration.widget({
//         widget: new StackedTypeExpectations<I>(fst, snd),
//         side: 0,
//     });
// };

// // FIXME this is a bad abstraction. Obviously, as I now need to copy the
// // whole thing to make it work for the [I, I, I, number] shaped type.
// let make_state_field_with_icon = <I extends Icon>(
//     ty: StateEffectType<Array<StackedPointIco<I>>>,
//     cons: (icos: Array<I>) => Decoration
// ) => {
//     return StateField.define<DecorationSet>({
//         create: () => Decoration.none,
//         update(points, transactions) {
//             console.log(transactions);
//             for (let e of transactions.effects) {
//                 if (e.is(ty)) {
//                     return RangeSet.of(e.value.map(([ico_o, ico_m, from]) =>
//                         cons([ico_o, ico_m]).range(from)));
//                 }
//             }

//             return points;
//         },
//         provide: f => EditorView.decorations.from(f),
//     })
// };

// let call_types_to_stacked_color = (own_shape: string, mut_shape: string) => {
//     return (call_type: ty.CallTypes): StackedPointIco<ColorDiffIco> => {
//         let high_color: RGB = `rgba(${112},${128},${144})`;
//         let low_color: RGB = `rgba(${233},${236},${238})`;
//         let color = (b: boolean) => (b ? high_color : low_color);
//         let ts_actual: ty.TypeState = call_type.actual.of_type;
//         let ts_expected: ty.TypeState = call_type.expected.of_type;
//         let [a_o, a_m] =
//             (("Owned" in ts_actual) ?
//                 [true, ts_actual.Owned.mutably_bound] :
//                 [false, ts_actual.Ref.is_mut]);
//         let [e_o, e_m] =
//             (("Owned" in ts_expected) ?
//                 [true, ts_expected.Owned.mutably_bound] :
//                 [false, ts_expected.Ref.is_mut]);
//         let owned_ico = new ColorDiffIco(own_shape, color(e_o), color(a_o));
//         let mut_ico = new ColorDiffIco(mut_shape, color(e_m), color(a_m));
//         let loc = call_type_to_loc(call_type);
//         return [owned_ico, mut_ico, loc];

//     }
// };

// // -------------------------
// // Icons with a state change
// // (e.g. locks that are locked vs unlocked)

// class StateDiffIco implements Icon {
//     constructor(
//         readonly state_expected: string,
//         readonly state_actual: string,
//         readonly color_expected: Color,
//         readonly color_actual: Color
//     ) {}

//     toDom(): HTMLElement {
//         let make_name_at_size = (ico_name: string) =>
//             (n: number) => `fa-solid ${ico_name} fa-stack-${n}x`;
//         let wrap = document.createElement("span");
//         wrap.className = "fa-stack small";
//         let box_o = wrap.appendChild(document.createElement("i"));
//         let box_i = wrap.appendChild(document.createElement("i"));
//         box_o.className = make_name_at_size(this.state_expected)(2);
//         box_i.className = make_name_at_size(this.state_actual)(2);
//         box_o.setAttribute("style", `color: ${this.color_expected};`)
//         box_i.setAttribute("style", `color: ${this.color_actual};`)
//         return wrap;
//     }
// };

// let stacked_state_ico_type = StateEffect.define<Array<StackedPointIco<StateDiffIco>>>();

// let call_types_to_stacked_state = (
//     own_shape: string,
//     ref_shape: string,
//     mut_shape: string,
//     immut_shape: string
// ) => {
//     return (call_type: ty.CallTypes): StackedPointIco<StateDiffIco> => {
//         let high_color: RGB = `rgba(${255},${0},${0})`;
//         let low_color: RGB = `rgba(${233},${236},${238})`;
//         let ts_actual: ty.TypeState = call_type.actual.of_type;
//         let ts_expected: ty.TypeState = call_type.expected.of_type;
//         let [a_o, a_m] =
//             (("Owned" in ts_actual) ?
//                 [true, ts_actual.Owned.mutably_bound] :
//                 [false, ts_actual.Ref.is_mut]);
//         let [e_o, e_m] =
//             (("Owned" in ts_expected) ?
//                 [true, ts_expected.Owned.mutably_bound] :
//                 [false, ts_expected.Ref.is_mut]);
//         let owned_ico = new StateDiffIco(
//             e_o ? own_shape : ref_shape,
//             a_o ? own_shape : ref_shape,
//             high_color, low_color
//         );
//         let mut_ico = new StateDiffIco(
//             e_m ? mut_shape : immut_shape,
//             a_m ? mut_shape : immut_shape,
//             high_color, low_color
//         );

//         let loc = call_type_to_loc(call_type);
//         return [owned_ico, mut_ico, loc];

//     }
// };

// // ---------------
// // RWD Permissions

// // -------------------
// // Exported interfaces

// export let hands_and_bans_field = {
//     effect_type: stacked_state_ico_type,
//     state_field: make_state_field_with_icon(
//         stacked_state_ico_type,
//         make_decoration_with_icon<StateDiffIco>
//     ),
//     make_decoration: make_decoration_with_icon<StateDiffIco>,
//     from_call_types: call_types_to_stacked_state(
//         "fa-hand-holding-droplet",
//         "fa-hand-holding",
//         "fa-circle",
//         "fa-ban"
//     ),
// };

// export let hands_and_shields_field = {
//     effect_type: stacked_state_ico_type,
//     state_field: make_state_field_with_icon(
//         stacked_state_ico_type,
//         make_decoration_with_icon<StateDiffIco>
//     ),
//     make_decoration: make_decoration_with_icon<StateDiffIco>,
//     from_call_types: call_types_to_stacked_state(
//         "fa-hands-holding-circle",
//         "fa-hands-holding",
//         "fa-shield",
//         "fa-shield-halved"
//     ),
// };

// export let square_circle_field: IconField<ColorDiffIco, StackedPointIco<ColorDiffIco>> = {
//     effect_type: stacked_color_ico_type,
//     state_field: make_state_field_with_icon(
//         stacked_color_ico_type,
//         make_decoration_with_icon<ColorDiffIco>
//     ),
//     make_decoration: make_decoration_with_icon<ColorDiffIco>,
//     from_call_types: call_types_to_stacked_color("fa-square", "fa-circle"),
// };

// // ------------------------
// // Widget types
// // (these control the display of the icons)

// class StackedTypeExpectations<I extends Icon> extends WidgetType {
//     constructor(readonly owner: I, readonly mut: I) { super() }

//     eq(other: StackedTypeExpectations<I>) {
//         return other.owner == this.owner && other.mut == this.mut;
//     }

//     toDOM() {
//         let wrap = document.createElement("span");
//         let l_ico = this.owner.toDom();
//         let r_ico = this.mut.toDom();
//         wrap.appendChild(l_ico);
//         wrap.appendChild(r_ico);
//         return wrap;
//     }

//     ignoreEvent() { return false }
// }
