//! `sdk-macros` —— 插件 SDK 的过程宏。
//!
//! 目的：让插件作者写 100% safe Rust，宏自动生成 `unsafe extern "C"`
//! ABI 入口 + serde (de)serialize + 内存 pack。
//!
//! 详见 `docs/PLUGIN_DEV.md` §3.0 「为什么看不到 unsafe」。

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, ItemFn, ReturnType, Type};

/// `#[plugin_export]` —— 把一个 safe Rust fn 包装成 wasm ABI 入口。
///
/// 形态：
///
/// ```ignore
/// #[plugin_export]
/// fn translate(req: TranslateRequest) -> String { /* ... */ }
/// ```
///
/// 展开为（伪代码）：
///
/// ```ignore
/// fn __plugin_inner_translate(req: TranslateRequest) -> String { /* ... */ }
///
/// #[no_mangle]
/// pub unsafe extern "C" fn translate(ptr: *mut u8, len: usize) -> u64 {
///     let bytes = ::sdk::read_input(ptr, len);
///     let req: TranslateRequest = match ::serde_json::from_slice(bytes) {
///         Ok(v) => v,
///         Err(_) => return ::sdk::pack_output(::std::vec::Vec::new()),
///     };
///     let result = __plugin_inner_translate(req);
///     ::sdk::pack_output(result.into_bytes())
/// }
/// ```
///
/// 支持的 fn 形态：
/// - 0 参数：忽略宿主传入字节
/// - 1 参数（任意 `serde::Deserialize` 类型）：自动 `from_slice`，失败返回空输出
///
/// 返回类型自动分派：
/// - `String` / `&str` → `pack_output(bytes)`
/// - `Vec<u8>` → `pack_output`
/// - 其他任意 `serde::Serialize` → `pack_json`（包括 `PluginManifest` 等）
///
/// 不支持 `Result<T, E>`（v1）—— 错误请在 fn 内自己编码进返回 JSON。
#[proc_macro_attribute]
pub fn plugin_export(_attr: TokenStream, item: TokenStream) -> TokenStream {
  let input = parse_macro_input!(item as ItemFn);

  let vis = input.vis.clone();
  let sig = input.sig.clone();
  let block = input.block.clone();
  let fn_name = sig.ident.clone();
  let fn_name_str = fn_name.to_string();
  let inner_name = syn::Ident::new(&format!("__plugin_inner_{}", fn_name_str), fn_name.span());

  if sig.asyncness.is_some() {
    return syn::Error::new_spanned(&sig, "#[plugin_export] does not support async fn")
      .to_compile_error()
      .into();
  }
  if sig.unsafety.is_some() {
    return syn::Error::new_spanned(
      &sig,
      "#[plugin_export] target fn must be safe; the macro generates the unsafe ABI wrapper",
    )
    .to_compile_error()
    .into();
  }

  let inputs: Vec<FnArg> = sig.inputs.iter().cloned().collect();
  if inputs.len() > 1 {
    return syn::Error::new_spanned(
      &sig.inputs,
      "#[plugin_export] supports 0 or 1 arguments (use a struct to bundle multi-field inputs)",
    )
    .to_compile_error()
    .into();
  }

  let (input_binding, decode_block, call_args): (
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    Vec<proc_macro2::TokenStream>,
  ) = match inputs.first() {
    None => (
      // 0 参数：让 ptr/len 名义上"被用过"以免 unused warning，但忽略其内容
      quote! { let _ = (ptr, len); },
      quote! {},
      vec![],
    ),
    Some(FnArg::Typed(pat_type)) => {
      let pat = pat_type.pat.clone();
      let ty = pat_type.ty.clone();
      (
        quote! { let __input_bytes: &[u8] = ::sdk::read_input(ptr, len); },
        quote! {
          let #pat: #ty = match ::serde_json::from_slice(__input_bytes) {
            ::core::result::Result::Ok(v) => v,
            ::core::result::Result::Err(_) => {
              return ::sdk::pack_output(::std::vec::Vec::new());
            }
          };
        },
        vec![quote! { #pat }],
      )
    }
    Some(FnArg::Receiver(rec)) => {
      return syn::Error::new_spanned(rec, "#[plugin_export] cannot be applied to methods")
        .to_compile_error()
        .into();
    }
  };

  let pack_call = match &sig.output {
    ReturnType::Default => {
      return syn::Error::new_spanned(
        &sig,
        "#[plugin_export] requires an explicit return type (no `-> ()`)",
      )
      .to_compile_error()
      .into();
    }
    ReturnType::Type(_, ty) => dispatch_pack(ty),
  };

  let inner_sig = {
    let mut s = sig.clone();
    s.ident = inner_name.clone();
    s
  };

  let expanded = quote! {
    #[doc(hidden)]
    #[allow(non_snake_case)]
    #inner_sig #block

    #[no_mangle]
    #[allow(clippy::missing_safety_doc)]
    #vis unsafe extern "C" fn #fn_name(ptr: *mut u8, len: usize) -> u64 {
      #input_binding
      #decode_block
      let __plugin_result = #inner_name( #(#call_args),* );
      #pack_call
    }
  };

  expanded.into()
}

/// 根据返回类型字面量决定打包路径。
///
/// 字面 `String` / `&str` / `&'static str` / `Vec<u8>` → `pack_output`，
/// 否则 → `pack_json`（要求实现 `Serialize`，编译期由 trait bound 强制）。
fn dispatch_pack(ty: &Type) -> proc_macro2::TokenStream {
  if matches_ident(ty, "String") {
    quote! { ::sdk::pack_output(::std::string::String::into_bytes(__plugin_result)) }
  } else if matches_str_ref(ty) {
    quote! { ::sdk::pack_output(__plugin_result.as_bytes().to_vec()) }
  } else if matches_vec_u8(ty) {
    quote! { ::sdk::pack_output(__plugin_result) }
  } else {
    quote! { ::sdk::pack_json(&__plugin_result) }
  }
}

fn matches_ident(ty: &Type, name: &str) -> bool {
  if let Type::Path(tp) = ty {
    if let Some(seg) = tp.path.segments.last() {
      return seg.ident == name;
    }
  }
  false
}

fn matches_str_ref(ty: &Type) -> bool {
  if let Type::Reference(r) = ty {
    if let Type::Path(tp) = &*r.elem {
      if let Some(seg) = tp.path.segments.last() {
        return seg.ident == "str";
      }
    }
  }
  false
}

fn matches_vec_u8(ty: &Type) -> bool {
  if let Type::Path(tp) = ty {
    if let Some(seg) = tp.path.segments.last() {
      if seg.ident == "Vec" {
        if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
          if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
            return matches_ident(inner, "u8");
          }
        }
      }
    }
  }
  false
}
