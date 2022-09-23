export type ContractInfo = {
  contract_info: {
    name: string;
    symbol: string;
  };
};

export type ContractConfig = {
  contract_config: {
    burn_is_enabled: boolean;
    minter_may_update_metadata: boolean;
    owner_is_public: boolean;
    owner_may_update_metadata: boolean;
    sealed_metadata_is_enabled: boolean;
    token_supply_is_public: boolean;
    unwrapped_metadata_is_private: boolean;
  };
};

export type Minters = {
  minters: {
    minters: HumanAddr[];
  };
};

export type NumTokens = {
  num_tokens: {
    count: number;
  };
};

export type TokenList = {
  token_list: {
    tokens: string[];
  };
};

export type OwnerOf = {
  owner_of: {
    approvals: Cw721Approval[];
    owner: HumanAddr;
  };
};

export type TokenApprovals = {
  token_approvals: {
    owner_is_public: boolean;
    private_metadata_is_public: boolean;
    private_metadata_is_public_expiration?: Expiration | null;
    public_ownership_expiration?: Expiration | null;
    token_approvals: Snip721Approval[];
  };
};

export type InventoryApprovals = {
  inventory_approvals: {
    inventory_approvals: Snip721Approval[];
    owner_is_public: boolean;
    private_metadata_is_public: boolean;
    private_metadata_is_public_expiration?: Expiration | null;
    public_ownership_expiration?: Expiration | null;
  };
};

export type NftInfo = {
  nft_info: {
    extension?: Extension | null;
    token_uri?: string | null;
  };
};

export type PrivateMetadata = {
  private_metadata: {
    extension?: Extension | null;
    token_uri?: string | null;
  };
};

export type AllNftInfo = {
  all_nft_info: {
    access: Cw721OwnerOfResponse;
    info?: Metadata | null;
  };
};

export type NftDossier = {
  nft_dossier: {
    display_private_metadata_error?: string | null;
    inventory_approvals?: Snip721Approval[] | null;
    mint_run_info?: MintRunInfo | null;
    owner?: HumanAddr | null;
    owner_is_public: boolean;
    private_metadata?: Metadata | null;
    private_metadata_is_public: boolean;
    private_metadata_is_public_expiration?: Expiration | null;
    public_metadata?: Metadata | null;
    public_ownership_expiration?: Expiration | null;
    royalty_info?: DisplayRoyaltyInfo | null;
    token_approvals?: Snip721Approval[] | null;
    transferable: boolean;
    unwrapped: boolean;
  };
};

export type BatchNftDossier = {
  batch_nft_dossier: {
    nft_dossiers: BatchNftDossierElement[];
  };
};

export type ApprovedForAll = {
  approved_for_all: {
    operators: Cw721Approval[];
  };
};

export type IsUnwrapped = {
  is_unwrapped: {
    token_is_unwrapped: boolean;
  };
};

export type IsTranferable = {
  is_transferable: {
    token_is_transferable: boolean;
  };
};

export type ImplementsNonTransferableTokens = {
  implements_non_transferable_tokens: {
    is_enabled: boolean;
  };
};

export type ImplementsTokenSubtype = {
  implements_token_subtype: {
    is_enabled: boolean;
  };
};

export type VerifyTransferApproval = {
  verify_transfer_approval: {
    approved_for_all: boolean;
    first_unapproved_token?: string | null;
  };
};

export type TransactionHistory = {
  transaction_history: {
    /**
     * total transaction count
     */
    total: number;
    txs: Tx[];
  };
};

export type RegisterCodeHash = {
  registered_code_hash: {
    also_implements_batch_receive_nft: boolean;
    code_hash?: string | null;
  };
};

export type RoyaltyInfo = {
  royalty_info: {
    royalty_info?: DisplayRoyaltyInfo | null;
  };
};

export type ContractCreator = {
  contract_creator: {
    creator?: HumanAddr | null;
  };
};

export type HumanAddr = string;

/**
 * at the given point in time and after, Expiration will be considered expired
 */
export type Expiration =
  | "never"
  | {
    at_height: number;
  }
  | {
    at_time: number;
  };

/**
 * tx type and specifics
 */
export type TxAction =
  | {
    transfer: {
      /**
       * previous owner
       */
      from: HumanAddr;
      /**
       * new owner
       */
      recipient: HumanAddr;
      /**
       * optional sender if not owner
       */
      sender?: HumanAddr | null;
    };
  }
  | {
    mint: {
      /**
       * minter's address
       */
      minter: HumanAddr;
      /**
       * token's first owner
       */
      recipient: HumanAddr;
    };
  }
  | {
    burn: {
      /**
       * burner's address if not owner
       */
      burner?: HumanAddr | null;
      /**
       * previous owner
       */
      owner: HumanAddr;
    };
  };

/**
 * CW721 Approval
 */
export interface Cw721Approval {
  /**
   * expiration of this approval
   */
  expires: Expiration;
  /**
   * address that can transfer the token
   */
  spender: HumanAddr;
}

/**
 * SNIP721 Approval
 */
export interface Snip721Approval {
  /**
   * whitelisted address
   */
  address: HumanAddr;
  /**
   * optional expiration if the address has transfer permission
   */
  transfer_expiration?: Expiration | null;
  /**
   * optional expiration if the address has view owner permission
   */
  view_owner_expiration?: Expiration | null;
  /**
   * optional expiration if the address has view private metadata permission
   */
  view_private_metadata_expiration?: Expiration | null;
}

/**
 * metadata extension You can add any metadata fields you need here.  These fields are based on https://docs.opensea.io/docs/metadata-standards and are the metadata fields that Stashh uses for robust NFT display.  Urls should be prefixed with `http://`, `https://`, `ipfs://`, or `ar://`
 */
export interface Extension {
  /**
   * url to a multimedia attachment
   */
  animation_url?: string | null;
  /**
   * item attributes
   */
  attributes?: Trait[] | null;
  /**
   * background color represented as a six-character hexadecimal without a pre-pended #
   */
  background_color?: string | null;
  /**
   * item description
   */
  description?: string | null;
  /**
   * url to allow users to view the item on your site
   */
  external_url?: string | null;
  /**
   * url to the image
   */
  image?: string | null;
  /**
   * raw SVG image data (not recommended). Only use this if you're not including the image parameter
   */
  image_data?: string | null;
  /**
   * media files as specified on Stashh that allows for basic authenticatiion and decryption keys. Most of the above is used for bridging public eth NFT metadata easily, whereas `media` will be used when minting NFTs on Stashh
   */
  media?: MediaFile[] | null;
  /**
   * name of the item
   */
  name?: string | null;
  /**
   * a select list of trait_types that are in the private metadata.  This will only ever be used in public metadata
   */
  protected_attributes?: string[] | null;
  /**
   * token subtypes used by Stashh for display groupings (primarily used for badges, which are specified by using "badge" as the token_subtype)
   */
  token_subtype?: string | null;
  /**
   * url to a YouTube video
   */
  youtube_url?: string | null;
}

/**
 * attribute trait
 */
export interface Trait {
  /**
   * indicates how a trait should be displayed
   */
  display_type?: string | null;
  /**
   * optional max value for numerical traits
   */
  max_value?: string | null;
  /**
   * name of the trait
   */
  trait_type?: string | null;
  /**
   * trait value
   */
  value: string;
}

/**
 * media file
 */
export interface MediaFile {
  /**
   * authentication information
   */
  authentication?: Authentication | null;
  /**
   * file extension
   */
  extension?: string | null;
  /**
   * file type Stashh currently uses: "image", "video", "audio", "text", "font", "application"
   */
  file_type?: string | null;
  /**
   * url to the file.  Urls should be prefixed with `http://`, `https://`, `ipfs://`, or `ar://`
   */
  url: string;
}

/**
 * media file authentication
 */
export interface Authentication {
  /**
   * either a decryption key for encrypted files or a password for basic authentication
   */
  key?: string | null;
  /**
   * username used in basic authentication
   */
  user?: string | null;
}

/**
 * response of CW721 OwnerOf
 */
export interface Cw721OwnerOfResponse {
  /**
   * list of addresses approved to transfer this token
   */
  approvals: Cw721Approval[];
  /**
   * Owner of the token if permitted to view it
   */
  owner?: HumanAddr | null;
}

/**
 * token metadata
 */
export interface Metadata {
  /**
   * optional on-chain metadata.  Only use this if you are not using `token_uri`
   */
  extension?: Extension | null;
  /**
   * optional uri for off-chain metadata.  This should be prefixed with `http://`, `https://`, `ipfs://`, or `ar://`.  Only use this if you are not using `extension`
   */
  token_uri?: string | null;
}

/**
 * information about the minting of the NFT
 */
export interface MintRunInfo {
  /**
   * optional address of the SNIP-721 contract creator
   */
  collection_creator?: HumanAddr | null;
  /**
   * optional number of the mint run this token was minted in.  A mint run represents a batch of NFTs released at the same time.  So if a creator decided to make 100 copies of an NFT, they would all be part of mint run number 1.  If they sold quickly, and the creator wanted to rerelease that NFT, he could make 100 more copies which would all be part of mint run number 2.
   */
  mint_run?: number | null;
  /**
   * optional total number of NFTs minted on this run.  This is used to represent that this token is number m of n
   */
  quantity_minted_this_run?: number | null;
  /**
   * optional serial number in this mint run.  This is used to serialize identical NFTs
   */
  serial_number?: number | null;
  /**
   * optional time of minting (in seconds since 01/01/1970)
   */
  time_of_minting?: number | null;
  /**
   * optional address of this NFT's creator
   */
  token_creator?: HumanAddr | null;
}

/**
 * display all royalty information
 */
export interface DisplayRoyaltyInfo {
  /**
   * decimal places in royalty rates
   */
  decimal_places_in_rates: number;
  /**
   * list of royalties
   */
  royalties: DisplayRoyalty[];
}

/**
 * display for a single royalty
 */
export interface DisplayRoyalty {
  /**
   * royalty rate
   */
  rate: number;
  /**
   * address to send royalties to.  Can be None to keep addresses private
   */
  recipient?: HumanAddr | null;
}

/**
 * the token id and nft dossier info of a single token response in a batch query
 */
export interface BatchNftDossierElement {
  display_private_metadata_error?: string | null;
  inventory_approvals?: Snip721Approval[] | null;
  mint_run_info?: MintRunInfo | null;
  owner?: HumanAddr | null;
  owner_is_public: boolean;
  private_metadata?: Metadata | null;
  private_metadata_is_public: boolean;
  private_metadata_is_public_expiration?: Expiration | null;
  public_metadata?: Metadata | null;
  public_ownership_expiration?: Expiration | null;
  royalty_info?: DisplayRoyaltyInfo | null;
  token_approvals?: Snip721Approval[] | null;
  token_id: string;
  /**
   * true if this token is transferable
   */
  transferable: boolean;
  /**
   * true if this token is unwrapped (returns true if the contract does not have selaed metadata enabled)
   */
  unwrapped: boolean;
}

/**
 * tx for display
 */
export interface Tx {
  /**
   * tx type and specifics
   */
  action: TxAction;
  /**
   * the block containing this tx
   */
  block_height: number;
  /**
   * the time (in seconds since 01/01/1970) of the block containing this tx
   */
  block_time: number;
  /**
   * optional memo
   */
  memo?: string | null;
  /**
   * token id
   */
  token_id: string;
  /**
   * tx id
   */
  tx_id: number;
}
