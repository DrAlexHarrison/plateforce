/* R's build machinery needs a C source in src/ before it will link the static library,
   and routine registration is forwarded from here so the linker keeps the archive. */

void R_init_plateforce_extendr(void *dll);
void register_extendr_panic_hook(void);

void R_init_plateforce(void *dll) {
    register_extendr_panic_hook();
    R_init_plateforce_extendr(dll);
}
