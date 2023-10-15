
def complete_config(cfg: dict, default: dict):
    """Complete config."""
    if type(cfg) != dict:
        return cfg
    
    for k in default:
        if k not in cfg:
            cfg[k] = default[k]
        elif isinstance(default[k], dict):
            complete_config(cfg[k], default[k])
    return cfg