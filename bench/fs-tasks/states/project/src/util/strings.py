def slug(s):
    return s.lower().replace(' ', '-')


def title(s):
    # TODO: keep acronyms upper case
    return s.title()
